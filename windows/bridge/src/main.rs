#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use clap::Parser;
use iht_protocol::{FLAG_TRACKING, PACKET_SIZE, PosePacket, quaternion_to_degrees};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{TcpStream, UdpSocket},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_dialog::MessageDialogButtons;
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg(windows)]
mod windows_webview;

const REPOSITORY_URL: &str = "https://github.com/alepouna/kitsutrack";
const ISSUE_URL: &str = "https://github.com/alepouna/kitsutrack/issues/new/choose";
const RELEASE_URL: &str = "https://api.github.com/repos/alepouna/kitsutrack/releases/latest";
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const DATA_DIRECTORY: &str = "KitsuTrack";
const MENU_FOCUS_LOSS_SETTLE_DELAY: Duration = Duration::from_millis(150);
static LOGS_WINDOW_BUILDING: AtomicBool = AtomicBool::new(false);

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
    /// Open the status panel with simulated tracking data instead of starting the bridge.
    #[arg(long)]
    preview: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    id: u64,
    session: String,
    timestamp: String,
    level: Level,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogFileSummary {
    session: String,
    started_at: String,
    size: u64,
    entries: usize,
    warnings: usize,
    errors: usize,
    current: bool,
}

struct Logger {
    file: Mutex<File>,
    session_dir: PathBuf,
    session: String,
    next_id: Mutex<u64>,
    repeated: Mutex<HashMap<String, (Instant, u32)>>,
}
struct StatusItem {
    text: Mutex<String>,
}
struct BridgeStats {
    tracking_rate: Mutex<Option<u32>>,
}
struct UpdateItem {
    url: Mutex<Option<String>>,
}
struct ChildProcess(Mutex<Option<Child>>);

trait RecoverLock<T> {
    fn recover(&self, context: &str) -> MutexGuard<'_, T>;
}

impl<T> RecoverLock<T> for Mutex<T> {
    fn recover(&self, context: &str) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(error) => {
                eprintln!("Recovered poisoned lock ({context}): {error}");
                error.into_inner()
            }
        }
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

fn main() {
    let args = Args::parse();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(move |app| Ok(setup(app.handle().clone(), args.clone())?))
        .invoke_handler(tauri::generate_handler![
            log_files,
            log_file,
            export_all_logs,
            reveal_log_file,
            delete_log_file,
            delete_all_log_files,
            menu_state,
            open_logs,
            client_error,
            open_about,
            report_feedback,
            check_for_updates,
            open_update_command,
            quit,
        ])
        .run(tauri::generate_context!())
        .expect("run KitsuTrack Bridge");
}

fn setup(app: AppHandle, args: Args) -> Result<()> {
    let logger = Arc::new(create_logger(&app, &args)?);
    app.manage(logger.clone());
    install_panic_logging(&app);
    app.manage(ChildProcess(Mutex::new(None)));

    app.manage(StatusItem {
        text: Mutex::new("Disconnected".into()),
    });
    app.manage(BridgeStats {
        tracking_rate: Mutex::new(None),
    });
    app.manage(UpdateItem {
        url: Mutex::new(None),
    });
    let icon = app_icon()?;
    if let Err(error) = TrayIconBuilder::new()
        .icon(icon)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left | MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                show_menu(tray.app_handle());
            }
        })
        .build(&app)
    {
        log(
            &app,
            Level::Error,
            format!("Could not initialize tray icon: {error}"),
        );
        return Err(error.into());
    }

    log(
        &app,
        Level::Info,
        format!("KitsuTrack Bridge {} started", env!("CARGO_PKG_VERSION")),
    );
    if args.preview {
        set_status(&app, "Connected / Tracking");
        *app.state::<BridgeStats>()
            .tracking_rate
            .recover("tracking rate lock") = Some(60);
        log(&app, Level::Info, "USB tunnel connected to Aurora's iPhone");
        log(
            &app,
            Level::Info,
            "Forwarding tracking data to OpenTrack at 60 FPS",
        );
        log(
            &app,
            Level::Warning,
            "Preview diagnostic: a delayed tracking frame was recovered",
        );
        show_menu(&app);
        show_logs(&app);
    } else {
        let update_app = app.clone();
        thread::spawn(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                if automatic_update_check_due(&update_app) {
                    check_updates(update_app.clone(), false);
                }
            }));
            if let Err(payload) = result {
                log(
                    &update_app,
                    Level::Error,
                    format!(
                        "Automatic update-check worker panicked: {}",
                        panic_message(payload)
                    ),
                );
            }
        });
        thread::spawn(move || run_bridge(app, args));
    }
    Ok(())
}

fn run_bridge(app: AppHandle, args: Args) {
    loop {
        let result = panic::catch_unwind(AssertUnwindSafe(|| run_bridge_session(&app, &args)));
        if let Err(payload) = result {
            log(
                &app,
                Level::Error,
                format!("Tracking worker panicked: {}", panic_message(payload)),
            );
            stop_helper(&app);
            set_status(&app, "Disconnected");
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn run_bridge_session(app: &AppHandle, args: &Args) {
    if let Err(error) = verify_windows_usbmux(args) {
        set_status(app, "Disconnected");
        log(app, Level::Error, format!("{error:#}"));
        thread::sleep(Duration::from_secs(5));
        return;
    }
    loop {
        start_helper(app, args);
        set_status(app, "Disconnected");
        *app.state::<BridgeStats>()
            .tracking_rate
            .recover("tracking rate lock") = None;
        log(
            app,
            Level::Info,
            format!("Connecting to USB tunnel at {}", args.source),
        );
        match forward(app, args) {
            Ok(()) => log(app, Level::Warning, "Tracker disconnected"),
            Err(error) => log(app, Level::Warning, format!("Connection error: {error:#}")),
        }
        set_status(app, "Disconnected");
        *app.state::<BridgeStats>()
            .tracking_rate
            .recover("tracking rate lock") = None;
        thread::sleep(Duration::from_secs(1));
    }
}

fn forward(app: &AppHandle, args: &Args) -> Result<()> {
    let udp = UdpSocket::bind("127.0.0.1:0").context("bind UDP output")?;
    let mut tcp = TcpStream::connect(&args.source).context("connect to USB forwarding helper")?;
    tcp.set_read_timeout(Some(Duration::from_millis(args.stale_ms)))?;
    tcp.set_nodelay(true)?;
    set_status(app, "Connected / Waiting for Tracking Data");
    log(
        app,
        Level::Info,
        "USB tunnel connected; waiting for iPhone tracking data",
    );
    let mut wire = [0_u8; PACKET_SIZE];
    let mut last_sequence = None;
    let mut forwarding = false;
    let mut rate_started = Instant::now();
    let mut rate_count = 0_u32;
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
            set_status(app, "Connected / Tracking");
            log(
                app,
                Level::Info,
                format!("Forwarding to OpenTrack at {}", args.opentrack),
            );
        }
        rate_count += 1;
        if rate_started.elapsed() >= Duration::from_secs(1) {
            *app.state::<BridgeStats>()
                .tracking_rate
                .recover("tracking rate lock") = Some(rate_count);
            let _ = app.emit("menu-state", menu_state(app.clone()));
            rate_started = Instant::now();
            rate_count = 0;
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
    let mut child_slot = child_process.0.recover("helper lock");
    if let Some(child) = child_slot.as_mut() {
        match child.try_wait() {
            Ok(None) => return,
            Ok(Some(status)) => log(
                app,
                if status.success() {
                    Level::Info
                } else {
                    Level::Warning
                },
                format!("USB forwarding helper exited with {status}"),
            ),
            Err(error) => log(
                app,
                Level::Warning,
                format!("Could not inspect USB forwarding helper: {error}"),
            ),
        }
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
    let mut command = Command::new(&executable);
    command
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    match command.spawn() {
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
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            for line in BufReader::new(output).lines() {
                match line {
                    Ok(line) => log(
                        &app,
                        Level::Info,
                        format!("{label}: {}", normalize_helper_output(&line)),
                    ),
                    Err(error) => {
                        log(
                            &app,
                            Level::Warning,
                            format!("Could not read {label} output: {error}"),
                        );
                        break;
                    }
                }
            }
        }));
        if let Err(payload) = result {
            log(
                &app,
                Level::Error,
                format!("{label} output worker panicked: {}", panic_message(payload)),
            );
        }
    });
}

fn stop_helper(app: &AppHandle) {
    if let Some(mut child) = app.state::<ChildProcess>().0.recover("helper lock").take() {
        if let Err(error) = child.kill() {
            log(
                app,
                Level::Warning,
                format!("Could not stop USB forwarding helper: {error}"),
            );
        }
        if let Err(error) = child.wait() {
            log(
                app,
                Level::Warning,
                format!("Could not wait for USB forwarding helper: {error}"),
            );
        }
    }
}

fn show_logs(app: &AppHandle) {
    log(app, Level::Info, "Logs window requested");
    if let Some(window) = app.get_webview_window("logs") {
        if let Err(error) = window.show().and_then(|_| window.set_focus()) {
            log(
                app,
                Level::Warning,
                format!("Could not show logs window: {error}"),
            );
        }
        log(app, Level::Info, "Existing logs window shown");
        return;
    }

    if LOGS_WINDOW_BUILDING.swap(true, Ordering::AcqRel) {
        log(
            app,
            Level::Warning,
            "Logs WebView2 window creation is already in progress",
        );
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(|| create_logs_window(&app)));
        if let Err(payload) = result {
            log(
                &app,
                Level::Error,
                format!(
                    "Logs WebView2 creation panicked: {}",
                    panic_message(payload)
                ),
            );
        }
        LOGS_WINDOW_BUILDING.store(false, Ordering::Release);
    });
}

fn create_logs_window(app: &AppHandle) {
    log(app, Level::Info, "Creating logs WebView2 window");
    let mut builder = WebviewWindowBuilder::new(app, "logs", WebviewUrl::App("index.html".into()))
        .title("KitsuTrack Bridge Logs")
        .inner_size(820.0, 540.0)
        .min_inner_size(560.0, 320.0)
        // Keep the window hidden until the native WebView2 failure handler is installed.
        // Showing it during build can expose a WebView2 startup failure before recovery
        // is attached.
        .visible(false);
    if let Some(directory) = webview_data_directory(app) {
        log(
            app,
            Level::Info,
            format!("Logs WebView2 data directory: {}", directory.display()),
        );
        builder = builder.data_directory(directory);
    }
    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            log(
                app,
                Level::Error,
                format!("Could not create logs window: {error}"),
            );
            return;
        }
    };

    let event_app = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { .. } => {
            log(&event_app, Level::Info, "Logs window close requested")
        }
        WindowEvent::Destroyed => log(&event_app, Level::Warning, "Logs window destroyed"),
        WindowEvent::Focused(focused) => log(
            &event_app,
            Level::Info,
            format!("Logs window focus changed: {focused}"),
        ),
        _ => {}
    });
    log(app, Level::Info, "Logs WebView2 window created");

    #[cfg(windows)]
    windows_webview::install_process_failed_recovery(&window);
    if let Err(error) = window.show().and_then(|_| window.set_focus()) {
        log(
            app,
            Level::Warning,
            format!("Could not show logs window after WebView2 setup: {error}"),
        );
    } else {
        log(app, Level::Info, "Logs window shown after WebView2 setup");
    }
    #[cfg(not(windows))]
    let _ = window;
}

fn show_menu(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("menu") {
        if window.is_visible().unwrap_or(false) {
            if let Err(error) = window.hide() {
                log(
                    app,
                    Level::Warning,
                    format!("Could not hide tray menu: {error}"),
                );
            }
            return;
        }
        show_menu_window(app, &window);
        return;
    }
    let icon = match app_icon() {
        Ok(icon) => icon,
        Err(error) => {
            log(
                app,
                Level::Error,
                format!("Could not load tray menu icon: {error}"),
            );
            return;
        }
    };
    let mut builder = match WebviewWindowBuilder::new(
        app,
        "menu",
        WebviewUrl::App("menu.html".into()),
    )
    .icon(icon)
    {
        Ok(builder) => builder,
        Err(error) => {
            log(
                app,
                Level::Error,
                format!("Could not create tray menu window: {error}"),
            );
            return;
        }
    };
    if let Some(directory) = webview_data_directory(app) {
        log(
            app,
            Level::Info,
            format!("Menu WebView2 data directory: {}", directory.display()),
        );
        builder = builder.data_directory(directory);
    }
    #[cfg(windows)]
    {
        // Transparent, undecorated windows can become click-through after a
        // WebView2 restart on Windows. Keep the tray UI opaque and interactive.
        builder = builder.transparent(false);
    }
    match builder
        .title("KitsuTrack Bridge")
        .inner_size(360.0, 280.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map(|window| {
            #[cfg(windows)]
            if let Err(error) = windows_webview::configure_menu_popup(&window) {
                log(
                    app,
                    Level::Warning,
                    format!("Could not configure tray menu popup: {error}"),
                );
            }
            let event_app = app.clone();
            let popup_has_been_focused = Arc::new(AtomicBool::new(false));
            let event_popup_has_been_focused = popup_has_been_focused.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { .. } => {
                    log(&event_app, Level::Info, "Tray menu close requested")
                }
                WindowEvent::Destroyed => {
                    log(&event_app, Level::Warning, "Tray menu window destroyed")
                }
                WindowEvent::Focused(false) => {
                    let app = event_app.clone();
                    let has_been_focused = event_popup_has_been_focused.load(Ordering::Acquire);
                    thread::spawn(move || {
                        thread::sleep(MENU_FOCUS_LOSS_SETTLE_DELAY);
                        let Some(window) = app.get_webview_window("menu") else {
                            return;
                        };
                        let is_visible = window.is_visible().unwrap_or(false);
                        let is_focused = window.is_focused().unwrap_or(false);
                        if should_hide_menu_after_focus_loss(
                            has_been_focused,
                            is_visible,
                            is_focused,
                        ) && let Err(error) = window.hide()
                        {
                            log(
                                &app,
                                Level::Warning,
                                format!("Could not hide tray menu after losing focus: {error}"),
                            );
                        }
                    });
                }
                WindowEvent::Focused(true) => {
                    event_popup_has_been_focused.store(true, Ordering::Release);
                    log(&event_app, Level::Info, "Tray menu focused")
                }
                _ => {}
            });
            #[cfg(windows)]
            windows_webview::install_process_failed_recovery(&window);
            show_menu_window(app, &window);
        }) {
        Ok(()) => {}
        Err(error) => log(
            app,
            Level::Warning,
            format!("Could not show tray menu window: {error}"),
        ),
    }
}

fn should_hide_menu_after_focus_loss(
    has_been_focused: bool,
    is_visible: bool,
    is_focused: bool,
) -> bool {
    has_been_focused && is_visible && !is_focused
}

fn show_menu_window(app: &AppHandle, window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    if let Err(error) = window.set_ignore_cursor_events(false) {
        log(
            app,
            Level::Warning,
            format!("Could not enable tray menu mouse input: {error}"),
        );
    }
    if let Err(error) = window.set_enabled(true) {
        log(
            app,
            Level::Warning,
            format!("Could not enable tray menu window: {error}"),
        );
    }
    if let Err(error) = window.move_window_constrained(Position::TrayCenter) {
        log(
            app,
            Level::Warning,
            format!("Could not position tray menu: {error}"),
        );
    }
    if let Err(error) = window.show() {
        log(
            app,
            Level::Warning,
            format!("Could not show tray menu: {error}"),
        );
    }
    if let Err(error) = window.set_focus() {
        log(
            app,
            Level::Warning,
            format!("Could not focus tray menu: {error}"),
        );
    }
}

fn hide_menu(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("menu")
        && let Err(error) = window.hide()
    {
        log(
            app,
            Level::Warning,
            format!("Could not close tray menu: {error}"),
        );
    }
}

fn webview_data_directory(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resolve(
            format!("{DATA_DIRECTORY}/WebView2"),
            BaseDirectory::LocalData,
        )
        .ok()
}

fn app_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(include_bytes!(
        "../../../shared/assets/kitsutrack/icon-64.png"
    ))
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn show_about(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .message(format!(
            "KitsuTrack Bridge {}\n\n{REPOSITORY_URL}",
            env!("CARGO_PKG_VERSION")
        ))
        .title("About KitsuTrack")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open GitHub".into(),
            "Close".into(),
        ))
        .show(move |open_github| {
            if open_github && let Err(error) = open::that(REPOSITORY_URL) {
                log(
                    &app,
                    Level::Warning,
                    format!("Could not open GitHub repository: {error}"),
                );
            }
        });
}

fn check_updates(app: AppHandle, manual: bool) {
    thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(|| match latest_release() {
            Ok(release) => {
                if let Ok(path) = update_check_path(&app)
                    && let Err(error) = fs::write(path, unix_millis().to_string())
                {
                    log(
                        &app,
                        Level::Warning,
                        format!("Could not record update check time: {error}"),
                    );
                }
                if is_newer_release(&release.tag_name) {
                    let update = app.state::<UpdateItem>();
                    *update.url.recover("update lock") = Some(release.html_url);
                    let _ = app.emit("menu-state", menu_state(app.clone()));
                    log(
                        &app,
                        Level::Info,
                        format!("Update available: {}", release.tag_name),
                    );
                    let _ = manual;
                } else if manual {
                    log(&app, Level::Info, "KitsuTrack Bridge is up to date");
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
            }
        }));
        if let Err(payload) = result {
            log(
                &app,
                Level::Error,
                format!("Update worker panicked: {}", panic_message(payload)),
            );
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

fn update_check_path(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .resolve(
            format!("{DATA_DIRECTORY}/last-update-check"),
            BaseDirectory::LocalData,
        )
        .context("resolve update check path")
}

fn automatic_update_check_due(app: &AppHandle) -> bool {
    let Ok(path) = update_check_path(app) else {
        log(
            app,
            Level::Warning,
            "Could not resolve update check path; checking for updates",
        );
        return true;
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
        .is_none_or(|last_check| {
            unix_millis().saturating_sub(last_check) >= UPDATE_CHECK_INTERVAL.as_millis()
        })
}

fn open_update(app: &AppHandle) {
    let update = app.state::<UpdateItem>();
    if let Some(url) = update.url.recover("update lock").as_deref()
        && let Err(error) = open::that(url)
    {
        log(
            app,
            Level::Warning,
            format!("Could not open update page: {error}"),
        );
    }
}

#[derive(Clone, Serialize)]
struct MenuState {
    status: String,
    iphone: String,
    tracking_rate: Option<u32>,
    update_available: bool,
}

#[tauri::command]
fn menu_state(app: AppHandle) -> MenuState {
    MenuState {
        status: app
            .state::<StatusItem>()
            .text
            .recover("status lock")
            .clone(),
        iphone: "iPhone connected".into(),
        tracking_rate: *app
            .state::<BridgeStats>()
            .tracking_rate
            .recover("tracking rate lock"),
        update_available: app
            .state::<UpdateItem>()
            .url
            .recover("update lock")
            .is_some(),
    }
}

#[tauri::command]
fn open_logs(app: AppHandle) {
    hide_menu(&app);
    show_logs(&app);
}

#[tauri::command]
fn client_error(app: AppHandle, message: String) {
    log(
        &app,
        Level::Error,
        format!("Frontend error: {}", message.trim()),
    );
}

#[tauri::command]
fn open_about(app: AppHandle) {
    hide_menu(&app);
    show_about(&app);
}

#[tauri::command]
fn report_feedback(app: AppHandle) {
    hide_menu(&app);
    let _ = open::that(ISSUE_URL);
}

#[tauri::command]
fn check_for_updates(app: AppHandle) {
    check_updates(app, true);
}

#[tauri::command]
fn open_update_command(app: AppHandle) {
    hide_menu(&app);
    open_update(&app);
}

#[tauri::command]
fn quit(app: AppHandle) {
    log(&app, Level::Info, "Bridge shutdown requested");
    stop_helper(&app);
    app.exit(0);
}

#[tauri::command]
fn log_files(logger: tauri::State<'_, Arc<Logger>>) -> Vec<LogFileSummary> {
    let mut files = fs::read_dir(&logger.session_dir)
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();
    files.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    files
        .into_iter()
        .filter_map(|file| {
            let session = file
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if !valid_session(&session) || !file.file_type().ok()?.is_file() {
                return None;
            }
            let entries = read_session(&file.path(), &session);
            if entries.is_empty() {
                return None;
            }
            Some(LogFileSummary {
                started_at: entries
                    .first()
                    .map(|entry| entry.timestamp.clone())
                    .unwrap_or_else(|| session.clone()),
                size: file.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                warnings: entries
                    .iter()
                    .filter(|entry| matches!(entry.level, Level::Warning))
                    .count(),
                errors: entries
                    .iter()
                    .filter(|entry| matches!(entry.level, Level::Error))
                    .count(),
                entries: entries.len(),
                current: session == logger.session,
                session,
            })
        })
        .collect()
}

#[tauri::command]
fn log_file(logger: tauri::State<'_, Arc<Logger>>, session: String) -> Vec<LogEntry> {
    if !valid_session(&session) {
        return Vec::new();
    }
    read_session(&session_path(&logger.session_dir, &session), &session)
}

#[tauri::command]
fn export_all_logs(app: AppHandle) {
    let logger = app.state::<Arc<Logger>>().inner().clone();
    app.dialog().file().pick_folder(move |folder| {
        let Some(folder) = folder.and_then(|path| path.as_path().map(PathBuf::from)) else {
            return;
        };
        let destination = folder.join(format!("kitsutrack-bridge-logs-{}.zip", unix_millis()));
        let Ok(file) = File::create(&destination) else {
            log(
                &app,
                Level::Error,
                format!("Could not create log archive {}", destination.display()),
            );
            return;
        };
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut sessions = fs::read_dir(&logger.session_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry
                    .file_type()
                    .map(|kind| kind.is_file())
                    .unwrap_or(false)
                    && entry.path().extension().and_then(|ext| ext.to_str()) == Some("log")
                    && entry
                        .path()
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(valid_session)
            })
            .collect::<Vec<_>>();
        sessions.sort_by_key(|entry| entry.file_name());
        for session_file in sessions {
            let session = session_file
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if !valid_session(&session)
                || !session_file
                    .file_type()
                    .map(|kind| !kind.is_file())
                    .unwrap_or(true)
            {
                continue;
            }
            let entries = read_session(&session_file.path(), &session);
            if let Err(error) = archive.start_file(format!("kitsutrack-{session}.log"), options) {
                log(
                    &app,
                    Level::Error,
                    format!("Could not add {session} to log archive: {error}"),
                );
                continue;
            }
            for entry in entries {
                if let Err(error) = writeln!(
                    archive,
                    "{} {:<7} {}",
                    entry.timestamp,
                    level_name(&entry.level),
                    entry.message
                ) {
                    log(
                        &app,
                        Level::Error,
                        format!("Could not write {session} to log archive: {error}"),
                    );
                    break;
                }
            }
        }
        if let Err(error) = archive.finish() {
            log(
                &app,
                Level::Error,
                format!("Could not finalize log archive: {error}"),
            );
        } else {
            log(
                &app,
                Level::Info,
                format!("Exported logs to {}", destination.display()),
            );
        }
    });
}

#[tauri::command]
fn reveal_log_file(logger: tauri::State<'_, Arc<Logger>>, session: String) -> Result<(), String> {
    let path = validated_session_path(&logger.session_dir, &session)?;
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg("-R").arg(&path).spawn();
    #[cfg(windows)]
    let result = {
        let mut command = Command::new("explorer.exe");
        command.creation_flags(CREATE_NO_WINDOW);
        command.arg(format!("/select,{}", path.display())).spawn()
    };
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(&logger.session_dir).spawn();
    result.map(|_| ()).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_log_file(logger: tauri::State<'_, Arc<Logger>>, session: String) -> Result<(), String> {
    let path = validated_session_path(&logger.session_dir, &session)?;
    if session == logger.session {
        let mut file = logger.file.recover("log file lock");
        file.set_len(0).map_err(|error| error.to_string())?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
    } else if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn delete_all_log_files(logger: tauri::State<'_, Arc<Logger>>) -> Result<(), String> {
    for entry in fs::read_dir(&logger.session_dir)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("log")
        {
            continue;
        }
        if entry
            .path()
            .file_stem()
            .is_some_and(|name| name == logger.session.as_str())
        {
            let mut file = logger.file.recover("log file lock");
            file.set_len(0).map_err(|error| error.to_string())?;
            file.seek(SeekFrom::Start(0))
                .map_err(|error| error.to_string())?;
        } else {
            fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn create_logger(app: &AppHandle, _args: &Args) -> Result<Logger> {
    let session_dir = app.path().resolve(
        format!("{DATA_DIRECTORY}/sessions"),
        BaseDirectory::LocalData,
    )?;
    fs::create_dir_all(&session_dir)?;
    migrate_legacy_logs(app, &session_dir);
    let mut sessions = fs::read_dir(&session_dir)?
        .flatten()
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("log")
                && entry
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(valid_session)
        })
        .collect::<Vec<_>>();
    sessions.sort_by_key(|entry| entry.file_name());
    for old in sessions.into_iter().rev().skip(9) {
        let _ = fs::remove_file(old.path());
    }
    let session = unix_millis().to_string();
    let file_path = session_dir.join(format!("{session}.log"));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)?;
    Ok(Logger {
        file: Mutex::new(file),
        session_dir,
        session,
        next_id: Mutex::new(1),
        repeated: Mutex::new(HashMap::new()),
    })
}

fn migrate_legacy_logs(app: &AppHandle, session_dir: &Path) {
    let Ok(legacy_dir) = app.path().resolve("sessions", BaseDirectory::AppLocalData) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&legacy_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(session) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|session| valid_session(session))
        else {
            continue;
        };
        if path.extension().and_then(|extension| extension.to_str()) != Some("log")
            || !entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        {
            continue;
        }
        let destination = session_path(session_dir, session);
        if destination.exists() {
            continue;
        }
        if let Err(error) = fs::rename(&path, &destination)
            .or_else(|_| fs::copy(&path, &destination).and_then(|_| fs::remove_file(&path)))
        {
            eprintln!("Could not migrate legacy log {}: {error}", path.display());
        }
    }
}

fn install_panic_logging(app: &AppHandle) {
    let app = app.clone();
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log(&app, Level::Error, format!("Application panic: {info}"));
        previous_hook(info);
    }));
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(ToString::to_string)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic payload".into())
}

fn log(app: &AppHandle, level: Level, message: impl Into<String>) {
    let logger = app.state::<Arc<Logger>>();
    let mut message = message.into();
    let key = message.clone();
    let mut repeated = logger.repeated.recover("repeated log lock");
    if let Some((last, count)) = repeated.get_mut(&key) {
        if last.elapsed() < Duration::from_secs(30) {
            *count += 1;
            return;
        }
        if *count > 0 {
            message = format!("{message} ({} repeats suppressed)", *count);
        }
    }
    repeated.insert(key, (Instant::now(), 0));
    drop(repeated);
    let mut next_id = logger.next_id.recover("log id lock");
    let entry = LogEntry {
        id: *next_id,
        session: logger.session.clone(),
        timestamp: timestamp(),
        level: level.clone(),
        message,
    };
    *next_id += 1;
    drop(next_id);
    if let Ok(line) = serde_json::to_string(&entry) {
        let mut file = logger.file.recover("log file lock");
        if let Err(error) = writeln!(file, "{line}") {
            eprintln!("Could not write application log entry: {error}");
        } else if matches!(level, Level::Warning | Level::Error) {
            let _ = file.flush();
        }
    } else {
        eprintln!("Could not serialize application log entry");
    }
    if let Err(error) = app.emit("log", entry) {
        eprintln!("Could not emit application log entry: {error}");
    }
}

fn normalize_helper_output(line: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(line) else {
        return line.to_string();
    };
    if let Some(object) = value.as_object_mut() {
        object.remove("time");
    }
    serde_json::to_string(&value).unwrap_or_else(|_| line.to_string())
}

/// Write a WebView2 diagnostic directly to the current application session file.
/// This avoids depending on frontend event delivery while the webview is broken.
#[cfg(windows)]
pub(crate) fn write_webview_diagnostic(app: &AppHandle, message: &str) {
    let logger = app.state::<Arc<Logger>>();
    let mut next_id = logger.next_id.recover("log id lock");
    let entry = LogEntry {
        id: *next_id,
        session: logger.session.clone(),
        timestamp: timestamp(),
        level: Level::Error,
        message: message.to_string(),
    };
    *next_id += 1;
    drop(next_id);

    match serde_json::to_string(&entry) {
        Ok(line) => {
            let mut file = logger.file.recover("log file lock");
            if let Err(error) = writeln!(file, "{line}").and_then(|_| file.flush()) {
                eprintln!("Could not write WebView2 diagnostic to session log: {error}");
            }
        }
        Err(error) => eprintln!("Could not serialize WebView2 diagnostic: {error}"),
    }
    let _ = app.emit("log", entry);
}

fn read_session(path: &std::path::Path, session: &str) -> Vec<LogEntry> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            serde_json::from_str(line)
                .ok()
                .or_else(|| parse_legacy_entry(line, session, index as u64 + 1))
        })
        .collect()
}

fn valid_session(session: &str) -> bool {
    !session.is_empty() && session.bytes().all(|byte| byte.is_ascii_digit())
}

fn session_path(session_dir: &std::path::Path, session: &str) -> PathBuf {
    session_dir.join(format!("{session}.log"))
}

fn validated_session_path(session_dir: &std::path::Path, session: &str) -> Result<PathBuf, String> {
    valid_session(session)
        .then(|| session_path(session_dir, session))
        .ok_or_else(|| "Invalid log session".to_string())
}

fn parse_legacy_entry(line: &str, session: &str, id: u64) -> Option<LogEntry> {
    let mut parts = line
        .splitn(3, char::is_whitespace)
        .filter(|part| !part.is_empty());
    let timestamp = parts.next()?.to_string();
    let level = match parts.next()? {
        "INFO" => Level::Info,
        "WARNING" => Level::Warning,
        "ERROR" => Level::Error,
        _ => return None,
    };
    Some(LogEntry {
        id,
        session: session.into(),
        timestamp,
        level,
        message: parts.next().unwrap_or_default().into(),
    })
}

fn level_name(level: &Level) -> &'static str {
    match level {
        Level::Info => "INFO",
        Level::Warning => "WARNING",
        Level::Error => "ERROR",
    }
}

fn set_status(app: &AppHandle, status: &str) {
    let item = app.state::<StatusItem>();
    *item.text.recover("status lock") = status.into();
    let _ = app.emit("menu-state", menu_state(app.clone()));
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

#[cfg(test)]
mod tests {
    use super::should_hide_menu_after_focus_loss;

    #[test]
    fn hides_only_after_a_settled_focus_loss() {
        assert!(!should_hide_menu_after_focus_loss(false, true, false));
        assert!(!should_hide_menu_after_focus_loss(true, false, false));
        assert!(!should_hide_menu_after_focus_loss(true, true, true));
        assert!(should_hide_menu_after_focus_loss(true, true, false));
    }
}
