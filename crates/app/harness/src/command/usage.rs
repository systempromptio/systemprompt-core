//! The usage text of the headless commands and every way an invocation can be
//! unusable before anything runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;

pub const USAGE: &str = "\
usage:
  run --workspace <dir> (--prompt <text> | --prompt-file <path>)...
      [--permission-mode ask|allow|deny] [--answers <path>]
      [--model <id>] [--agent <name>] [--data-root <dir>]
      [--sync-timeout <seconds>] [--timeout <seconds>]
  login --stdin --instance <url> [--fingerprint sha256:<hex>] [--data-root <dir>]";

/// Why an argument list, or what it names, cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("no command was given; expected run or login")]
    NoCommand,
    #[error("unknown command `{name}`; expected run or login")]
    UnknownCommand { name: String },
    #[error("`{command}` takes no argument `{argument}`")]
    UnknownArgument {
        command: &'static str,
        argument: String,
    },
    #[error("`{flag}` needs a value")]
    MissingValue { flag: &'static str },
    #[error("`{flag}` takes no value")]
    UnexpectedValue { flag: &'static str },
    #[error("`{flag}` was given more than once")]
    Repeated { flag: &'static str },
    #[error("`{command}` needs `{flag}`")]
    Missing {
        command: &'static str,
        flag: &'static str,
    },
    #[error("`{flag}`: `{value}` is not {expected}")]
    Invalid {
        flag: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("{} could not be read", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the answers file {} does not decode", path.display())]
    Answers {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{} is not a directory", path.display())]
    NotADirectory { path: PathBuf },
    #[error("no token was read from standard input")]
    EmptyToken,
    #[error("standard input could not be read")]
    Stdin(#[source] io::Error),
}
