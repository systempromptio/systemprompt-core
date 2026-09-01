//! `gui` command: launches the desktop GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;
use std::sync::Arc;

use crate::context::BridgeContext;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn cmd_gui(ctx: Arc<BridgeContext>) -> ExitCode {
    #[cfg(target_os = "windows")]
    {
        crate::winproc::detach_console();
        // Why: toasts and taskbar grouping are keyed on the AUMID, and without
        // an installer nothing else declares one for this process.
        crate::winproc::set_app_user_model_id(crate::brand::brand().aumid);
        if !crate::gui::webview2::ensure_present() {
            return ExitCode::FAILURE;
        }
    }
    let _guard = match crate::single_instance::try_acquire_gui() {
        crate::single_instance::SingletonResult::Acquired(g) => g,
        crate::single_instance::SingletonResult::AlreadyRunning => {
            if crate::single_instance::ping_focus_running_instance() {
                crate::stdio::diag(
                    "gui: another bridge instance is already running; focused its window",
                );
                return ExitCode::SUCCESS;
            }
            // Why: the lock holder is alive (flock releases on process death)
            // but never confirmed the focus request — it is wedged or GUI-less.
            // A second GUI would race the proxy and loopback ports, so refuse
            // visibly: on a Finder launch this is the difference between an
            // explanation and an app that appears to do nothing.
            let app = crate::brand::brand().app_name;
            crate::stdio::diag(
                "gui: another bridge instance holds the lock but did not answer the focus \
                 request; exiting",
            );
            crate::user_alert::alert_user(
                &format!("{app} is already running"),
                "It is not responding, so its window could not be opened. Quit it from the menu \
                 bar, then try again.",
            );
            return ExitCode::FAILURE;
        },
        crate::single_instance::SingletonResult::Error(e) => {
            crate::stdio::diag(&format!("gui: singleton check failed: {e}; continuing"));
            return crate::gui::run(ctx);
        },
    };
    let exit = crate::gui::run(ctx);
    crate::single_instance::clear_running_port();
    exit
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn cmd_gui(_ctx: Arc<BridgeContext>) -> ExitCode {
    crate::stdio::diag("gui not supported on this platform");
    ExitCode::from(64)
}
