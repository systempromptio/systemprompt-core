pub(crate) fn parse_opt_flag(args: &[String], flag: &str) -> Option<String> {
    let mut i = 2;
    while i < args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

pub(crate) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().skip(2).any(|a| a == flag)
}

#[cfg(target_os = "windows")]
pub(crate) fn launched_without_terminal() -> bool {
    use windows_sys::Win32::System::Console::GetConsoleProcessList;
    let mut pids = [0u32; 4];
    let n = unsafe { GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) };
    n <= 1
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn launched_without_terminal() -> bool {
    false
}
