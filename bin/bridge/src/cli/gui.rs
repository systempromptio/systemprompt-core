//! `gui` command: launches the desktop GUI. A newer build launched while an
//! older one runs takes over before the context binds the proxy port, so it
//! serves on the default port once the older instance has released it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;
use std::sync::Arc;

use crate::context::BridgeContext;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn cmd_gui(ctx: Arc<BridgeContext>, held: HeldLock) -> ExitCode {
    #[cfg(target_os = "windows")]
    {
        crate::winproc::detach_console();
        // Why: Windows keys toast identity and taskbar grouping on the process AUMID.
        crate::winproc::set_app_user_model_id(crate::brand::brand().aumid);
        if !crate::gui::webview2::ensure_present() {
            return ExitCode::FAILURE;
        }
    }
    let acquired = held.guard.map_or_else(
        crate::single_instance::try_acquire_gui,
        crate::single_instance::SingletonResult::Acquired,
    );
    let _guard = match acquired {
        crate::single_instance::SingletonResult::Acquired(g) => g,
        crate::single_instance::SingletonResult::AlreadyRunning => {
            if crate::single_instance::ping_focus_running_instance() {
                crate::stdio::diag(
                    "gui: another bridge instance is already running; focused its window",
                );
                return ExitCode::SUCCESS;
            }
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
            crate::stdio::diag(&format!(
                "gui: singleton check failed: {e}; refusing to start a second instance blind"
            ));
            return ExitCode::from(70);
        },
    };
    crate::update::staged::reconcile_for_running();
    let exit = crate::gui::run(ctx);
    if let Err(e) = crate::single_instance::clear_running_port() {
        crate::stdio::diag(&format!("clear bridge sidecar: {e}"));
        return ExitCode::FAILURE;
    }
    exit
}

#[derive(Default)]
pub(super) struct HeldLock {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    guard: Option<crate::single_instance::SingletonGuard>,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(super) fn take_over_older_instance() -> HeldLock {
    HeldLock {
        guard: crate::single_instance::take_over_older_instance(),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn take_over_older_instance() -> HeldLock {
    HeldLock::default()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn cmd_gui(_ctx: Arc<BridgeContext>, _held: HeldLock) -> ExitCode {
    crate::stdio::diag("gui not supported on this platform");
    ExitCode::from(64)
}
