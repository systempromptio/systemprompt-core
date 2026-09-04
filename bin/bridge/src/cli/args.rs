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

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn should_default_to_gui() -> bool {
    use is_terminal::IsTerminal as _;

    !std::io::stdout().is_terminal()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) const fn should_default_to_gui() -> bool {
    false
}

/// Collects every occurrence of a repeatable `--flag <value>` option.
///
/// Why: `parse_opt_flag` returns the first match only, which silently drops
/// every later `--host` on a line that names several. Values are additionally
/// split on commas so `--host a,b` and `--host a --host b` mean the same
/// thing, and blanks are dropped so a trailing comma is not an unknown id.
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
