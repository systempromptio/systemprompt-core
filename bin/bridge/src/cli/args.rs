//! Argument parsing helpers, including the GUI-by-default heuristic.
//!
//! Each command declares its flags as a [`CommandFlags`]; [`check_flags`]
//! refuses an argument list with an unknown flag, a value flag with no value,
//! or a flag where a value belongs (`--pubkey --apply`), before the lookup
//! helpers below read it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// The flags and positional arguments one command accepts.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandFlags {
    pub switches: &'static [&'static str],
    pub values: &'static [&'static str],
    pub positionals: usize,
    pub inline_values: bool,
}

/// Why an argument list was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    #[error("unknown flag `{0}`")]
    UnknownFlag(String),
    #[error("`{0}` needs a value")]
    MissingValue(String),
    #[error("`{flag}` needs a value but was followed by the flag `{found}`")]
    FlagAsValue { flag: String, found: String },
    #[error("unexpected argument `{0}`")]
    UnexpectedArgument(String),
}

pub fn check_flags(args: &[String], spec: &CommandFlags) -> Result<(), UsageError> {
    let mut positionals = 0;
    let mut i = 2;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if spec.switches.contains(&arg) {
            continue;
        }
        if spec.values.contains(&arg) {
            match args.get(i) {
                None => return Err(UsageError::MissingValue(arg.to_owned())),
                Some(next) if next.starts_with("--") => {
                    return Err(UsageError::FlagAsValue {
                        flag: arg.to_owned(),
                        found: next.clone(),
                    });
                },
                Some(_) => i += 1,
            }
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let inline = arg
                .split_once('=')
                .is_some_and(|(name, value)| !value.is_empty() && spec.values.contains(&name));
            if spec.inline_values && inline {
                continue;
            }
            return Err(UsageError::UnknownFlag(arg.to_owned()));
        }
        positionals += 1;
        if positionals > spec.positionals {
            return Err(UsageError::UnexpectedArgument(arg.to_owned()));
        }
    }
    Ok(())
}

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
