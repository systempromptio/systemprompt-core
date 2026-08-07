//! `gui` command: launches the desktop GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn cmd_gui() -> ExitCode {
    #[cfg(target_os = "windows")]
    crate::winproc::detach_console();
    let _guard = match crate::single_instance::try_acquire_gui() {
        crate::single_instance::SingletonResult::Acquired(g) => g,
        crate::single_instance::SingletonResult::AlreadyRunning => {
            if crate::single_instance::ping_focus_running_instance() {
                crate::obs::output::diag(
                    "gui: another bridge instance is already running; focused its window",
                );
                return ExitCode::SUCCESS;
            }
            // The lock is held (flock/named mutex release on process death, so the
            // holder is alive) but it never confirmed the focus request — it is
            // wedged, or was started without a GUI. Starting a second GUI would
            // race the proxy and loopback ports, so refuse; but say so visibly,
            // because on a Finder/Explorer launch this is the difference between
            // an explanation and an app that appears to do nothing at all.
            let app = crate::brand::brand().app_name;
            crate::obs::output::diag(
                "gui: another bridge instance holds the lock but did not answer the focus \
                 request; exiting",
            );
            crate::gui::window::notify_user(
                &format!("{app} is already running"),
                "It is not responding, so its window could not be opened. Quit it from the menu \
                 bar, then try again.",
            );
            return ExitCode::FAILURE;
        },
        crate::single_instance::SingletonResult::Error(e) => {
            crate::obs::output::diag(&format!("gui: singleton check failed: {e}; continuing"));
            return crate::gui::run();
        },
    };
    let exit = crate::gui::run();
    crate::single_instance::clear_running_port();
    exit
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn cmd_gui() -> ExitCode {
    crate::obs::output::diag("gui not supported on this platform");
    ExitCode::from(64)
}
