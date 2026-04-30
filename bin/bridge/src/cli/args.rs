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
pub(crate) fn should_default_to_gui() -> bool {
    use is_terminal::IsTerminal as _;

    !std::io::stdout().is_terminal()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn should_default_to_gui() -> bool {
    false
}
