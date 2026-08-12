#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use clap::Parser;
use iht_protocol::{FLAG_TRACKING, PACKET_SIZE, PosePacket, quaternion_to_degrees};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, UdpSocket},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    path::BaseDirectory,
    tray::TrayIconBuilder,
};
use tauri_plugin_dialog::DialogExt;

const REPOSITORY_URL: &str = "https://github.com/alepouna/kitsutrack";
const ISSUE_URL: &str = "https://github.com/alepouna/kitsutrack/issues/new/choose";
const RELEASE_URL: &str = "https://api.github.com/repos/alepouna/kitsutrack/releases/latest";
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Parser, Clone)]
#[command(about = "KitsuTrack USB bridge for OpenTrack")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:4243")]
    source: String,
    #[arg(long, default_value = "127.0.0.1:4242")]
    opentrack: String,
    #[arg(long, default_value_t = 500)]
    stale_ms: u64,
    #[arg(long)]
    no_usb_helper: bool,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Serialize)]
struct LogEntry {
    timestamp: String,
    level: Level,
    message: String,
}

struct Logger {
    entries: Mutex<Vec<LogEntry>>,
    file: Mutex<File>,
    session_dir: PathBuf,
    settings: String,
}
struct StatusItem {
    item: MenuItem<tauri::Wry>,
    text: Mutex<String>,
}
struct UpdateItem {
    item: MenuItem<tauri::Wry>,
    url: Mutex<Option<String>>,
    shown: Mutex<bool>,
}
struct TrayMenu(Menu<tauri::Wry>);
struct ChildProcess(Mutex<Option<Child>>);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

fn main() {
    let args = Args::parse();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| Ok(setup(app.handle().clone(), args.clone())?))
        .invoke_handler(tauri::generate_handler![logs, export_logs])
        .run(tauri::generate_context!())
        .expect("run KitsuTrack Bridge");
}

fn setup(app: AppHandle, args: Args) -> Result<()> {
    let logger = Arc::new(create_logger(&app, &args)?);
    app.manage(logger.clone());
    app.manage(ChildProcess(Mutex::new(None)));

    let status = MenuItem::with_id(&app, "status", "Starting…", false, None::<&str>)?;
    let logs = MenuItem::with_id(&app, "logs", "Logs", true, None::<&str>)?;
    let check_updates_item = MenuItem::with_id(
        &app,
        "check-updates",
        "Check for updates",
        true,
        None::<&str>,
    )?;
    let update = MenuItem::with_id(&app, "update", "", false, None::<&str>)?;
    let about = MenuItem::with_id(&app, "about", "About KitsuTrack", true, None::<&str>)?;
    let feedback = MenuItem::with_id(
        &app,
        "feedback",
        "Report a bug / suggest a feature",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(&app)?;
    let menu = Menu::with_items(
        &app,
        &[
            &status,
            &separator,
            &logs,
            &check_updates_item,
            &about,
            &feedback,
            &separator,
            &quit,
        ],
    )?;
    app.manage(StatusItem {
        item: status,
        text: Mutex::new("Starting…".into()),
    });
    app.manage(UpdateItem {
        item: update,
        url: Mutex::new(None),
        shown: Mutex::new(false),
    });
    app.manage(TrayMenu(menu.clone()));
    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "logs" => show_logs(app),
            "check-updates" => check_updates(app.clone(), true),
            "update" => open_update(app),
            "about" => show_about(app),
            "feedback" => {
                let _ = open::that(ISSUE_URL);
            }
            "quit" => {
                stop_helper(app);
                app.exit(0);
            }
            _ => {}
        })
        .build(&app)?;

    log(
        &app,
        Level::Info,
        format!("KitsuTrack Bridge {} started", env!("CARGO_PKG_VERSION")),
    );
    let update_app = app.clone();
    thread::spawn(move || {
        if automatic_update_check_due(&update_app) {
            check_updates(update_app, false);
        }
    });
    thread::spawn(move || run_bridge(app, args));
    Ok(())
}

fn run_bridge(app: AppHandle, args: Args) {
    if let Err(error) = verify_windows_usbmux(&args) {
        set_status(&app, "USB helper unavailable — view Logs");
        log(&app, Level::Error, format!("{error:#}"));
        return;
    }
    loop {
        start_helper(&app, &args);
        set_status(&app, "Connecting to iPhone via USB…");
        log(
            &app,
            Level::Info,
            format!("Connecting to USB tunnel at {}", args.source),
        );
        match forward(&app, &args) {
            Ok(()) => log(&app, Level::Warning, "Tracker disconnected"),
            Err(error) => log(&app, Level::Warning, format!("Connection error: {error:#}")),
        }
        set_status(&app, "Connection error — view Logs");
        thread::sleep(Duration::from_secs(1));
    }
}

fn forward(app: &AppHandle, args: &Args) -> Result<()> {
    let udp = UdpSocket::bind("127.0.0.1:0").context("bind UDP output")?;
    let mut tcp = TcpStream::connect(&args.source).context("connect to USB forwarding helper")?;
    tcp.set_read_timeout(Some(Duration::from_millis(args.stale_ms)))?;
    tcp.set_nodelay(true)?;
    set_status(app, "Waiting for tracking data");
    log(
        app,
        Level::Info,
        "USB tunnel connected; waiting for iPhone tracking data",
    );
    let mut wire = [0_u8; PACKET_SIZE];
    let mut last_sequence = None;
    let mut forwarding = false;
    loop {
        tcp.read_exact(&mut wire)
            .context("no tracking packets received")?;
        let packet = match PosePacket::decode(&wire) {
            Ok(packet) => packet,
            Err(error) => {
                log(
                    app,
                    Level::Warning,
                    format!("Discarded malformed packet: {error:?}"),
                );
                continue;
            }
        };
        if last_sequence.is_some_and(|last| packet.sequence <= last) {
            continue;
        }
        last_sequence = Some(packet.sequence);
        if packet.flags & FLAG_TRACKING == 0 {
            continue;
        }
        if !forwarding {
            forwarding = true;
            set_status(app, "Connected to iPhone via USB");
            log(
                app,
                Level::Info,
                format!("Forwarding to OpenTrack at {}", args.opentrack),
            );
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
        for (value, invert) in values.iter_mut().zip([
            args.invert_x,
            args.invert_y,
            args.invert_z,
            args.invert_yaw,
            args.invert_pitch,
            args.invert_roll,
        ]) {
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
    }
}

fn start_helper(app: &AppHandle, args: &Args) {
    let child_process = app.state::<ChildProcess>();
    let mut child_slot = child_process.0.lock().expect("helper lock");
    if child_slot
        .as_mut()
        .is_some_and(|child| child.try_wait().ok().flatten().is_none())
    {
        return;
    }
    *child_slot = None;
    if args.no_usb_helper || args.source != "127.0.0.1:4243" {
        return;
    }
    let Some(executable) = args.usb_tool.clone().or_else(find_usb_tool) else {
        log(
            app,
            Level::Warning,
            "USB forwarding tool was not found; tracker simulator can still be used",
        );
        return;
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                relay_helper_output(app.clone(), stdout, "USB helper");
            }
            if let Some(stderr) = child.stderr.take() {
                relay_helper_output(app.clone(), stderr, "USB helper");
            }
            log(
                app,
                Level::Info,
                format!("Started USB forwarding using {}", executable.display()),
            );
            *child_slot = Some(child);
        }
        Err(error) => log(
            app,
            Level::Error,
            format!("Could not start USB forwarding helper: {error}"),
        ),
    }
}

fn relay_helper_output<R: Read + Send + 'static>(app: AppHandle, output: R, label: &'static str) {
    thread::spawn(move || {
        for line in BufReader::new(output)
            .lines()
            .map_while(std::result::Result::ok)
        {
            log(&app, Level::Info, format!("{label}: {line}"));
        }
    });
}

fn stop_helper(app: &AppHandle) {
    if let Some(mut child) = app
        .state::<ChildProcess>()
        .0
        .lock()
        .expect("helper lock")
        .take()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn show_logs(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("logs") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "logs", WebviewUrl::App("index.html".into()))
        .title("KitsuTrack Bridge Logs")
        .inner_size(820.0, 540.0)
        .min_inner_size(560.0, 320.0)
        .build();
}

fn show_about(app: &AppHandle) {
    app.dialog()
        .message(format!(
            "KitsuTrack Bridge {}\n\n{REPOSITORY_URL}",
            env!("CARGO_PKG_VERSION")
        ))
        .title("About KitsuTrack")
        .show(|_| {});
}

fn check_updates(app: AppHandle, manual: bool) {
    thread::spawn(move || match latest_release() {
        Ok(release) => {
            let _ = fs::write(update_check_path(&app), unix_millis().to_string());
            if is_newer_release(&release.tag_name) {
                let update = app.state::<UpdateItem>();
                let _ = update
                    .item
                    .set_text(format!("Update available: {}", release.tag_name));
                let _ = update.item.set_enabled(true);
                *update.url.lock().expect("update lock") = Some(release.html_url);
                let mut shown = update.shown.lock().expect("update shown lock");
                if !*shown {
                    let _ = app.state::<TrayMenu>().0.append(&update.item);
                    *shown = true;
                }
                log(
                    &app,
                    Level::Info,
                    format!("Update available: {}", release.tag_name),
                );
                if manual {
                    show_temporary_status(&app, format!("Update available: {}", release.tag_name));
                }
            } else if manual {
                log(&app, Level::Info, "KitsuTrack Bridge is up to date");
                show_temporary_status(&app, "KitsuTrack Bridge is up to date");
            } else {
                log(
                    &app,
                    Level::Info,
                    "Update check completed; bridge is up to date",
                );
            }
        }
        Err(error) => {
            log(
                &app,
                Level::Warning,
                format!("Update check failed: {error:#}"),
            );
            if manual {
                show_temporary_status(&app, "Couldn't check for updates — view Logs");
            }
        }
    });
}

fn latest_release() -> Result<Release> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("KitsuTrack-Bridge/{}", env!("CARGO_PKG_VERSION")))
        .build()?
        .get(RELEASE_URL)
        .send()?
        .error_for_status()?
        .json()
        .context("parse GitHub release response")
}

fn is_newer_release(tag: &str) -> bool {
    let Ok(latest) = semver::Version::parse(tag.trim_start_matches('v')) else {
        return false;
    };
    let installed =
        semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is semver");
    latest > installed
}

fn update_check_path(app: &AppHandle) -> PathBuf {
    app.path()
        .resolve("last-update-check", BaseDirectory::AppLocalData)
        .expect("resolve update check path")
}

fn automatic_update_check_due(app: &AppHandle) -> bool {
    fs::read_to_string(update_check_path(app))
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
        .is_none_or(|last_check| {
            unix_millis().saturating_sub(last_check) >= UPDATE_CHECK_INTERVAL.as_millis()
        })
}

fn open_update(app: &AppHandle) {
    let update = app.state::<UpdateItem>();
    if let Some(url) = update.url.lock().expect("update lock").as_deref() {
        let _ = open::that(url);
    }
}

fn show_temporary_status(app: &AppHandle, text: impl Into<String>) {
    let temporary = text.into();
    let original = app
        .state::<StatusItem>()
        .text
        .lock()
        .expect("status lock")
        .clone();
    set_status(app, &temporary);
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        let status = app.state::<StatusItem>();
        if *status.text.lock().expect("status lock") == temporary {
            set_status(&app, &original);
        }
    });
}

#[tauri::command]
fn logs(logger: tauri::State<'_, Arc<Logger>>) -> Vec<LogEntry> {
    logger.entries.lock().expect("log lock").clone()
}

#[tauri::command]
fn export_logs(app: AppHandle) {
    let logger = app.state::<Arc<Logger>>().inner().clone();
    app.dialog()
        .file()
        .add_filter("Log file", &["log"])
        .set_file_name("kitsutrack-bridge-logs.log")
        .save_file(move |path| {
            let Some(path) = path else {
                return;
            };
            let text = export_text(&logger);
            if let Err(error) = fs::write(path.as_path().expect("local file path"), text) {
                log(
                    &app,
                    Level::Error,
                    format!("Could not export logs: {error}"),
                );
            } else {
                log(&app, Level::Info, "Logs exported");
            }
        });
}

fn create_logger(app: &AppHandle, args: &Args) -> Result<Logger> {
    let session_dir = app
        .path()
        .resolve("sessions", BaseDirectory::AppLocalData)?;
    fs::create_dir_all(&session_dir)?;
    let mut sessions = fs::read_dir(&session_dir)?.flatten().collect::<Vec<_>>();
    sessions.sort_by_key(|entry| entry.file_name());
    for old in sessions.into_iter().rev().skip(9) {
        let _ = fs::remove_file(old.path());
    }
    let started = unix_millis();
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(session_dir.join(format!("{started}.log")))?;
    Ok(Logger {
        entries: Mutex::new(Vec::new()),
        file: Mutex::new(file),
        session_dir,
        settings: format!(
            "USB tunnel: {}\nOpenTrack: {}\nInversions: x={} y={} z={} yaw={} pitch={} roll={}",
            args.source,
            args.opentrack,
            args.invert_x,
            args.invert_y,
            args.invert_z,
            args.invert_yaw,
            args.invert_pitch,
            args.invert_roll,
        ),
    })
}

fn log(app: &AppHandle, level: Level, message: impl Into<String>) {
    let entry = LogEntry {
        timestamp: timestamp(),
        level,
        message: message.into(),
    };
    let logger = app.state::<Arc<Logger>>();
    let line = format!(
        "{} {:<7} {}\n",
        entry.timestamp,
        match entry.level {
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
        },
        entry.message
    );
    let _ = logger
        .file
        .lock()
        .expect("log file lock")
        .write_all(line.as_bytes());
    logger.entries.lock().expect("log lock").push(entry.clone());
    let _ = app.emit("log", entry);
}

fn export_text(logger: &Logger) -> String {
    let mut output = format!(
        "KitsuTrack Bridge {}\nExported: {}\nWindows: {}\nArchitecture: {}\n{}\n\n",
        env!("CARGO_PKG_VERSION"),
        timestamp(),
        os_info::get(),
        env::consts::ARCH,
        logger.settings
    );
    let mut sessions = fs::read_dir(&logger.session_dir)
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();
    sessions.sort_by_key(|entry| entry.file_name());
    for session in sessions {
        output.push_str(&format!(
            "===== {} =====\n",
            session.file_name().to_string_lossy()
        ));
        output.push_str(&fs::read_to_string(session.path()).unwrap_or_default());
        output.push('\n');
    }
    output
}

fn set_status(app: &AppHandle, status: &str) {
    let item = app.state::<StatusItem>();
    *item.text.lock().expect("status lock") = status.into();
    let _ = item.item.set_text(status);
}
fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| unix_millis().to_string())
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
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    })
}
