use std::process::ExitCode;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn cmd_gui() -> ExitCode {
    #[cfg(target_os = "windows")]
    crate::winproc::detach_console();
    crate::gui::run()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn cmd_gui() -> ExitCode {
    crate::obs::output::diag("gui not supported on this platform");
    ExitCode::from(64)
}
