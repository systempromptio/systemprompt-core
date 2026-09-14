//! Argument parsing helpers, including the GUI-by-default heuristic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub fn parse_opt_flag(args: &[String], flag: &str) -> Option<String> {
    let mut i = 2;
    while i < args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

pub fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().skip(2).any(|a| a == flag)
}

#[cfg(target_os = "windows")]
pub(crate) fn launched_without_console() -> bool {
    !crate::winproc::attach_parent_console_if_present()
}

// Why: LaunchServices sets `__CFBundleIdentifier` only when it starts the
// app bundle (Finder, Dock, `open`); a shell, cron or launchd job does not.
#[cfg(target_os = "macos")]
pub(crate) fn launched_without_console() -> bool {
    std::env::var_os("__CFBundleIdentifier").is_some()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) const fn launched_without_console() -> bool {
    false
}

pub fn parse_multi_flag(args: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 2;
    while i < args.len() {
        if args[i] == flag && i + 1 < args.len() {
            for part in args[i + 1].split(',') {
                let part = part.trim();
                if !part.is_empty() && !out.iter().any(|v: &String| v == part) {
                    out.push(part.to_owned());
                }
            }
            i += 1;
        }
        i += 1;
    }
    out
}
