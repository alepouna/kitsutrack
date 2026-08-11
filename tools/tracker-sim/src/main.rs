use anyhow::{Context, Result};
use clap::Parser;
use iht_protocol::{FLAG_TRACKING, PosePacket};
use std::{
    f32::consts::PI,
    io::Write,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(about = "Synthetic iPhone tracking source using the production protocol")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:4243")]
    listen: String,
    #[arg(long, default_value_t = 60.0)]
    rate: f32,
    #[arg(long, default_value_t = 25.0)]
    amplitude_degrees: f32,
    #[arg(long, default_value_t = 0.2)]
    amplitude_metres: f32,
    #[arg(long, default_value_t = 0.25)]
    frequency: f32,
    #[arg(long, default_value_t = 0.0)]
    packet_loss: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.rate > 0.0 && (0.0..=1.0).contains(&args.packet_loss),
        "invalid rate or packet loss"
    );
    let listener =
        TcpListener::bind(&args.listen).with_context(|| format!("listen on {}", args.listen))?;
    eprintln!("synthetic tracker listening on {}", args.listen);
    loop {
        let (mut stream, peer) = listener.accept()?;
        stream.set_nodelay(true)?;
        eprintln!("bridge connected from {peer}");
        let start = Instant::now();
        let interval = Duration::from_secs_f32(1.0 / args.rate);
        for sequence in 0_u64.. {
            let t = start.elapsed().as_secs_f32();
            let phase = 2.0 * PI * args.frequency * t;
            let yaw = args.amplitude_degrees.to_radians() * phase.sin();
            let half = yaw / 2.0;
            let packet = PosePacket {
                flags: FLAG_TRACKING,
                sequence,
                timestamp_ns: start.elapsed().as_nanos() as u64,
                rotation: [0.0, half.sin(), 0.0, half.cos()],
                translation: [
                    args.amplitude_metres * phase.sin(),
                    args.amplitude_metres * (phase * 0.7).sin(),
                    args.amplitude_metres * (phase * 0.5).cos(),
                ],
            };
            // Deterministic loss makes diagnostics and tests reproducible.
            let drop_every = if args.packet_loss == 0.0 {
                0
            } else {
                (1.0 / args.packet_loss).round() as u64
            };
            if (drop_every == 0 || sequence % drop_every != 0)
                && stream.write_all(&packet.encode()).is_err()
            {
                break;
            }
            thread::sleep(interval.saturating_sub(start.elapsed() - Duration::from_secs_f32(t)));
        }
    }
}
