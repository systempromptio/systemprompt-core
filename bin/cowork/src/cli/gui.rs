use std::process::ExitCode;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn cmd_gui() -> ExitCode {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::FreeConsole;
        if crate::cli::args::launched_without_terminal() {
            FreeConsole();
        }
    }
    crate::gui::run()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn cmd_gui() -> ExitCode {
    crate::obs::output::diag("gui not supported on this platform");
    ExitCode::from(64)
}
