//! ARQ-06: reacts when a WebView2 process of the main window fails.
//!
//! A renderer crash used to leave no trace, and an instance whose browser
//! process was gone stayed alive without a window, so the single-instance
//! plugin sent every new launch to it. Each failure now writes a local report
//! in `crash-reports`; a dead or hung renderer brings the interface back, and
//! a dead browser process ends the instance so the next launch starts normally.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tauri::Manager;
use webview2_com::{
    take_pwstr, CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND,
    ProcessFailedEventHandler, SourceChangedEventHandler,
};

use crate::crash_report::{
    webview_failure_action, write_webview_failure_report, LockRecovering, WebviewFailureAction,
};

/// Exit code of an instance closed because its browser process failed.
const BROWSER_PROCESS_FAILED_EXIT_CODE: i32 = 3;

pub fn watch(app: &tauri::AppHandle, report_directory: PathBuf) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let handle = app.clone();
    let _ = window.with_webview(move |webview| {
        // SAFETY: the controller and its CoreWebView2 belong to this window
        // and are used on the thread that owns them, as wry does.
        unsafe {
            let Ok(core) = webview.controller().CoreWebView2() else {
                return;
            };
            // After a renderer crash the page is about:blank and Reload()
            // keeps it there, so recovery goes back to the last app URL.
            let last_app_url = Arc::new(Mutex::new(None::<String>));
            let mut current = Default::default();
            if core.Source(&mut current).is_ok() {
                let current = take_pwstr(current);
                if !current.is_empty() && !current.starts_with("about:") {
                    *last_app_url.lock_recovering() = Some(current);
                }
            }
            let remembered = Arc::clone(&last_app_url);
            let mut token = 0_i64;
            let _ = core.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(sender) = sender {
                        let mut source = Default::default();
                        if sender.Source(&mut source).is_ok() {
                            let source = take_pwstr(source);
                            if !source.is_empty() && !source.starts_with("about:") {
                                *remembered.lock_recovering() = Some(source);
                            }
                        }
                    }
                    Ok(())
                })),
                &mut token,
            );
            let _ = core.add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(move |sender, args| {
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                    if let Some(args) = args {
                        let _ = args.ProcessFailedKind(&mut kind);
                    }
                    let _ = write_webview_failure_report(&report_directory, kind.0);
                    match webview_failure_action(kind.0) {
                        WebviewFailureAction::Exit => handle.exit(BROWSER_PROCESS_FAILED_EXIT_CODE),
                        WebviewFailureAction::Reload => {
                            let url = last_app_url.lock_recovering().clone();
                            if let (Some(sender), Some(url)) = (sender, url) {
                                let url = CoTaskMemPWSTR::from(url.as_str());
                                let _ = sender.Navigate(*url.as_ref().as_pcwstr());
                            }
                        }
                        WebviewFailureAction::ReportOnly => {}
                    }
                    Ok(())
                })),
                &mut token,
            );
        }
    });
}
