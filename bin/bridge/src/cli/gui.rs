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
            } else {
                crate::obs::output::diag(
                    "gui: another bridge instance is already running; exiting",
                );
            }
            return ExitCode::SUCCESS;
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
pub(crate) fn cmd_gui() -> ExitCode {
    crate::obs::output::diag("gui not supported on this platform");
    ExitCode::from(64)
}
