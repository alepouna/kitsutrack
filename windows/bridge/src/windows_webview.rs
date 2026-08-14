//! Recover individual WebView2 windows without stopping the bridge worker.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use tauri::{Manager, WebviewWindow};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, ICoreWebView2ProcessFailedEventArgs,
    },
    ProcessFailedEventHandler,
};

const BROWSER_PROCESS_EXITED: i32 = 0;
const RELOADABLE_FAILURES: std::ops::RangeInclusive<i32> = 1..=3;
const RECREATE_ATTEMPTS: usize = 20;
const RECREATE_DELAY: Duration = Duration::from_millis(100);

/// Install WebView2 process-failure recovery for a bridge UI window.
pub fn install_process_failed_recovery(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let result_label = label.clone();
    let result_app = app.clone();
    let result = window.with_webview(move |platform| {
        let webview = match unsafe { platform.controller().CoreWebView2() } {
            Ok(webview) => webview,
            Err(error) => {
                super::log(
                    &app,
                    super::Level::Warning,
                    format!("WebView2 {label}: unable to access CoreWebView2: {error}"),
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
            super::log(
                &handler_app,
                super::Level::Error,
                format!(
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
            super::log(
                &app,
                super::Level::Warning,
                format!("WebView2 {label}: unable to install ProcessFailed handler: {error}"),
            );
        }
    });

    if let Err(error) = result {
        super::log(
            &result_app,
            super::Level::Warning,
            format!("WebView2 {result_label}: unable to access native webview: {error}"),
        );
    }
}

fn recreate_window(app: &tauri::AppHandle, label: &str) {
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
