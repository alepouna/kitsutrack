//! Recover individual WebView2 windows without stopping the bridge worker.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{Manager, WebviewWindow};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, ICoreWebView2ProcessFailedEventArgs,
    },
    ProcessFailedEventHandler,
};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_CAPTION, WS_EX_APPWINDOW,
        WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    },
};

const BROWSER_PROCESS_EXITED: i32 = 0;
const RELOADABLE_FAILURES: std::ops::RangeInclusive<i32> = 1..=3;
const RECREATE_ATTEMPTS: usize = 20;
const RECREATE_DELAY: Duration = Duration::from_millis(100);

/// Turn the already-created Tauri window into a native popup without using
/// transparency, which can make WebView2 hit testing unreliable after a
/// renderer restart.
pub fn configure_menu_popup(window: &WebviewWindow) -> tauri::Result<()> {
    let handle = window
        .window_handle()
        .map_err(|error| tauri::Error::Anyhow(anyhow::anyhow!(error.to_string())))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(tauri::Error::Anyhow(anyhow::anyhow!(
            "tray menu did not provide a Win32 window handle"
        )));
    };
    let hwnd = HWND(handle.hwnd.get() as _);

    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let popup_style = (style
            & !(WS_CAPTION.0
                | WS_THICKFRAME.0
                | WS_SYSMENU.0
                | WS_MINIMIZEBOX.0
                | WS_MAXIMIZEBOX.0))
            | WS_POPUP.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, popup_style as isize);

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let popup_ex_style = (ex_style & !WS_EX_APPWINDOW.0) | WS_EX_TOOLWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, popup_ex_style as isize);

        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );

        // Rounded corners are a best-effort Windows 11 enhancement. The
        // opaque HTML surface remains the fallback on older Windows builds.
        let preference = DWMWCP_ROUND.0;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as _,
            std::mem::size_of_val(&preference) as u32,
        );
    }
    Ok(())
}

/// Install WebView2 process-failure recovery for a bridge UI window.
pub fn install_process_failed_recovery(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let result_label = label.clone();
    let result_app = app.clone();
    let result = window.with_webview(move |platform| {
        super::log(
            &app,
            super::Level::Info,
            format!("WebView2 {label}: native webview initialization started"),
        );
        let webview = match unsafe { platform.controller().CoreWebView2() } {
            Ok(webview) => webview,
            Err(error) => {
                super::write_webview_diagnostic(
                    &app,
                    &format!("WebView2 {label}: unable to access CoreWebView2: {error}"),
                );
                return;
            }
        };

        let handler_app = app.clone();
        let handler_label = label.clone();
        let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
            let kind = args
                .as_ref()
                .and_then(|args: &ICoreWebView2ProcessFailedEventArgs| {
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                    unsafe { args.ProcessFailedKind(&mut kind) }
                        .ok()
                        .map(|()| kind.0)
                })
                .unwrap_or(-1);
            super::write_webview_diagnostic(
                &handler_app,
                &format!(
                    "WebView2 {handler_label} process failed (kind {kind}); attempting recovery"
                ),
            );
            let reload_succeeded = RELOADABLE_FAILURES.contains(&kind)
                && sender
                    .as_ref()
                    .and_then(|webview| unsafe { webview.Reload() }.ok())
                    .is_some();
            if kind == BROWSER_PROCESS_EXITED
                || RELOADABLE_FAILURES.contains(&kind) && !reload_succeeded
            {
                recreate_window(&handler_app, &handler_label);
            }
            Ok(())
        }));

        let mut token = 0i64;
        if let Err(error) = unsafe { webview.add_ProcessFailed(&handler, &mut token) } {
            super::write_webview_diagnostic(
                &app,
                &format!("WebView2 {label}: unable to install ProcessFailed handler: {error}"),
            );
        } else {
            super::log(
                &app,
                super::Level::Info,
                format!("WebView2 {label}: ProcessFailed handler installed (token {token})"),
            );
        }
    });

    if let Err(error) = result {
        super::write_webview_diagnostic(
            &result_app,
            &format!("WebView2 {result_label}: unable to access native webview: {error}"),
        );
    }
}

fn recreate_window(app: &tauri::AppHandle, label: &str) {
    super::log(
        app,
        super::Level::Warning,
        format!("WebView2 {label}: recovery started"),
    );
    let app = app.clone();
    let label = label.to_string();
    let main_app = app.clone();
    let initial_label = label.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Some(window) = main_app.get_webview_window(&initial_label) {
            if let Err(error) = window.close() {
                super::log(
                    &main_app,
                    super::Level::Warning,
                    format!("Could not close failed WebView2 window {initial_label}: {error}"),
                );
            }
        }
    }) {
        super::log(
            &app,
            super::Level::Warning,
            format!("Could not schedule WebView2 {label} recovery: {error}"),
        );
        return;
    }
    super::log(
        &app,
        super::Level::Info,
        format!("WebView2 {label}: close scheduled for failed window"),
    );

    let retry_app = app.clone();
    let recreated = Arc::new(AtomicBool::new(false));
    thread::spawn(move || {
        for attempt in 1..=RECREATE_ATTEMPTS {
            thread::sleep(RECREATE_DELAY);
            let recovery_app = retry_app.clone();
            let recovery_label = label.clone();
            let recreated_for_main = recreated.clone();
            if let Err(error) = retry_app.run_on_main_thread(move || {
                if let Some(window) = recovery_app.get_webview_window(&recovery_label) {
                    if let Err(error) = window.close() {
                        super::log(
                            &recovery_app,
                            super::Level::Warning,
                            format!(
                                "WebView2 {recovery_label} close retry {attempt} failed: {error}"
                            ),
                        );
                    }
                    return;
                }

                match recovery_label.as_str() {
                    "logs" => super::show_logs(&recovery_app),
                    "menu" => super::show_menu(&recovery_app),
                    _ => super::log(
                        &recovery_app,
                        super::Level::Warning,
                        format!("No recovery path for WebView2 window {recovery_label}"),
                    ),
                }
                if recovery_app.get_webview_window(&recovery_label).is_some() {
                    super::log(
                        &recovery_app,
                        super::Level::Info,
                        format!(
                            "WebView2 {recovery_label}: recovery recreated window on attempt {attempt}"
                        ),
                    );
                    recreated_for_main.store(true, Ordering::Release);
                }
            }) {
                super::log(
                    &retry_app,
                    super::Level::Warning,
                    format!("WebView2 {label} recovery attempt {attempt} could not run: {error}"),
                );
            }
            if recreated.load(Ordering::Acquire) {
                return;
            }
            if attempt == RECREATE_ATTEMPTS {
                super::log(
                    &retry_app,
                    super::Level::Error,
                    format!("WebView2 {label} recovery exhausted after {attempt} attempts"),
                );
            }
        }
    });
}
