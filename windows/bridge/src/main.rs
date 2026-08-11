use anyhow::{Context, Result};
use clap::Parser;
use iht_protocol::{FLAG_TRACKING, PACKET_SIZE, PosePacket, quaternion_to_degrees};
use std::{
    env,
    io::Read,
    net::{TcpStream, UdpSocket},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(about = "USB-tunnel to OpenTrack UDP bridge")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:4243")]
    source: String,
    #[arg(long, default_value = "127.0.0.1:4242")]
    opentrack: String,
    #[arg(long, default_value_t = 500)]
    stale_ms: u64,
    /// Do not automatically start a USB forwarding helper.
    #[arg(long)]
    no_usb_helper: bool,
    /// Explicit go-ios `ios.exe` or iproxy executable.
    #[arg(long)]
    usb_tool: Option<PathBuf>,
    #[arg(long)]
    invert_x: bool,
    #[arg(long)]
    invert_y: bool,
    #[arg(long)]
    invert_z: bool,
    #[arg(long)]
    invert_yaw: bool,
    #[arg(long)]
    invert_pitch: bool,
    #[arg(long)]
    invert_roll: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    verify_windows_usbmux(&args)?;
    let mut usb_tunnel = start_usb_tunnel(&args);
    let udp = UdpSocket::bind("127.0.0.1:0").context("bind UDP output")?;
    loop {
        if usb_tunnel
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            eprintln!("USB tunnel stopped; restarting it");
            usb_tunnel = start_usb_tunnel(&args);
            thread::sleep(Duration::from_secs(1));
        }
        eprintln!("connecting to USB tunnel at {}", args.source);
        match forward(&args, &udp) {
            Ok(()) => eprintln!("tracker disconnected"),
            Err(error) => eprintln!("connection error: {error:#}"),
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn verify_windows_usbmux(args: &Args) -> Result<()> {
    if !cfg!(windows) || args.no_usb_helper || args.source != "127.0.0.1:4243" {
        return Ok(());
    }
    let address = "127.0.0.1:27015".parse().expect("valid static address");
    if TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_err() {
        anyhow::bail!(
            "Apple USB device service is not running on 127.0.0.1:27015. Install/open Apple Devices for Windows, reconnect and trust the iPhone, then restart this bridge"
        );
    }
    Ok(())
}

fn forward(args: &Args, udp: &UdpSocket) -> Result<()> {
    let mut tcp = TcpStream::connect(&args.source).context("connect to USB forwarding helper")?;
    tcp.set_read_timeout(Some(Duration::from_millis(args.stale_ms)))?;
    tcp.set_nodelay(true)?;
    eprintln!("USB tunnel accepted the connection; waiting for iPhone tracking data");
    let mut wire = [0_u8; PACKET_SIZE];
    let mut last_sequence = None;
    let started = Instant::now();
    let mut count = 0_u64;
    let mut stream_active = false;
    loop {
        tcp.read_exact(&mut wire).context(
            "no tracking packets received (keep the iPhone app open and check the USB helper output)",
        )?;
        let packet = match PosePacket::decode(&wire) {
            Ok(packet) => packet,
            Err(error) => {
                eprintln!("discarded malformed packet: {error:?}");
                continue;
            }
        };
        if !stream_active {
            eprintln!(
                "iPhone tracking stream active; forwarding to OpenTrack at {}",
                args.opentrack
            );
            stream_active = true;
        }
        if last_sequence.is_some_and(|last| packet.sequence <= last) {
            continue;
        }
        last_sequence = Some(packet.sequence);
        if packet.flags & FLAG_TRACKING == 0 {
            continue;
        }
        let angles = quaternion_to_degrees(packet.rotation);
        let mut values = [
            packet.translation[0] as f64 * 100.0,
            packet.translation[1] as f64 * 100.0,
            packet.translation[2] as f64 * 100.0,
            angles[0],
            angles[1],
            angles[2],
        ];
        let inversions = [
            args.invert_x,
            args.invert_y,
            args.invert_z,
            args.invert_yaw,
            args.invert_pitch,
            args.invert_roll,
        ];
        for (value, invert) in values.iter_mut().zip(inversions) {
            if invert {
                *value = -*value;
            }
        }
        let mut datagram = [0_u8; 48];
        for (i, value) in values.iter().enumerate() {
            datagram[i * 8..i * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
        udp.send_to(&datagram, &args.opentrack)
            .context("send OpenTrack UDP")?;
        count += 1;
        if count.is_multiple_of(120) {
            let hz = count as f64 / started.elapsed().as_secs_f64();
            eprintln!(
                "{hz:.1} Hz | xyz cm [{:.1}, {:.1}, {:.1}] | ypr° [{:.1}, {:.1}, {:.1}]",
                values[0], values[1], values[2], values[3], values[4], values[5]
            );
        }
    }
}

fn start_usb_tunnel(args: &Args) -> Option<Child> {
    if args.no_usb_helper || args.source != "127.0.0.1:4243" {
        return None;
    }
    let executable = args.usb_tool.clone().or_else(find_usb_tool);
    let Some(executable) = executable else {
        eprintln!("USB forwarding tool was not found; continuing so tracker-sim can be used");
        return None;
    };
    let is_go_ios = executable
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("ios.exe"));
    let arguments: &[&str] = if is_go_ios {
        &["forward", "4243", "4243"]
    } else {
        &["4243", "4243"]
    };
    match Command::new(&executable)
        .args(arguments)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            eprintln!("started USB forwarding using {}", executable.display());
            Some(child)
        }
        Err(error) => {
            eprintln!("could not start USB forwarding helper: {error}");
            None
        }
    }
}

fn find_usb_tool() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["ios.exe", "iproxy.exe"]
    } else {
        &["ios", "iproxy"]
    };
    if let Ok(exe) = env::current_exe() {
        for name in names {
            let candidate = exe.parent()?.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    env::var_os("PATH").and_then(|path| {
        for directory in env::split_paths(&path) {
            for name in names {
                let candidate = directory.join(name);
                if Path::new(&candidate).is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    })
}
