//! The argument lists of the headless commands.
//!
//! `run` and `login` each take named arguments only, as `--name value` or
//! `--name=value`. [`parse`] is pure; [`RunArgs::load`] then reads the files
//! a `run` names and checks its workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::time::Duration;

use systemprompt_device::view::KeyFingerprint;

use crate::command::permission::{Answers, PermissionMode};
use crate::command::run::{DEFAULT_SYNC_TIMEOUT, RunRequest};
use crate::command::usage::UsageError;

const RUN: &str = "run";
const LOGIN: &str = "login";
const PROMPT: &str = "--prompt";
const PROMPT_FILE: &str = "--prompt-file";
const WORKSPACE: &str = "--workspace";
const PERMISSION_MODE: &str = "--permission-mode";
const ANSWERS: &str = "--answers";
const MODEL: &str = "--model";
const AGENT: &str = "--agent";
const DATA_ROOT: &str = "--data-root";
const SYNC_TIMEOUT: &str = "--sync-timeout";
const TIMEOUT: &str = "--timeout";
const STDIN: &str = "--stdin";
const INSTANCE: &str = "--instance";
const FINGERPRINT: &str = "--fingerprint";

const RUN_VALUED: [&str; 10] = [
    PROMPT,
    PROMPT_FILE,
    WORKSPACE,
    PERMISSION_MODE,
    ANSWERS,
    MODEL,
    AGENT,
    DATA_ROOT,
    SYNC_TIMEOUT,
    TIMEOUT,
];
const LOGIN_VALUED: [&str; 3] = [INSTANCE, FINGERPRINT, DATA_ROOT];
const LOGIN_SWITCHES: [&str; 1] = [STDIN];

/// A prompt given on the command line or in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSource {
    Text(String),
    File(PathBuf),
}

/// The arguments of `run`, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunArgs {
    pub prompts: Vec<PromptSource>,
    pub workspace: PathBuf,
    pub permission_mode: PermissionMode,
    pub answers: Option<PathBuf>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub data_root: Option<PathBuf>,
    pub sync_timeout: Duration,
    pub timeout: Option<Duration>,
}

/// The arguments of `login`. The token is never an argument: it is read from
/// standard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginArgs {
    pub instance: String,
    pub fingerprint: Option<KeyFingerprint>,
    pub data_root: Option<PathBuf>,
}

/// One headless command with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(Box<RunArgs>),
    Login(LoginArgs),
}

struct Given {
    flag: &'static str,
    value: Option<String>,
}

pub fn parse<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None => Err(UsageError::NoCommand),
        Some(RUN) => parse_run(collect(RUN, args, &RUN_VALUED, &[])?),
        Some(LOGIN) => parse_login(&collect(LOGIN, args, &LOGIN_VALUED, &LOGIN_SWITCHES)?),
        Some(name) => Err(UsageError::UnknownCommand {
            name: name.to_owned(),
        }),
    }
}

fn known(flags: &[&'static str], name: &str) -> Option<&'static str> {
    flags.iter().copied().find(|flag| *flag == name)
}

fn collect(
    command: &'static str,
    mut args: impl Iterator<Item = String>,
    valued: &[&'static str],
    switches: &[&'static str],
) -> Result<Vec<Given>, UsageError> {
    let mut given = Vec::new();
    while let Some(argument) = args.next() {
        let (name, inline) = match argument.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (argument.as_str(), None),
        };
        if let Some(flag) = known(valued, name) {
            let value = match inline {
                Some(value) => value,
                None => args.next().ok_or(UsageError::MissingValue { flag })?,
            };
            given.push(Given {
                flag,
                value: Some(value),
            });
        } else if let Some(flag) = known(switches, name) {
            if inline.is_some() {
                return Err(UsageError::UnexpectedValue { flag });
            }
            given.push(Given { flag, value: None });
        } else {
            return Err(UsageError::UnknownArgument { command, argument });
        }
    }
    Ok(given)
}

fn single(given: &[Given], flag: &'static str) -> Result<Option<String>, UsageError> {
    let mut values = given
        .iter()
        .filter(|entry| entry.flag == flag)
        .filter_map(|entry| entry.value.clone());
    let first = values.next();
    if values.next().is_some() {
        return Err(UsageError::Repeated { flag });
    }
    Ok(first)
}

fn required(
    given: &[Given],
    command: &'static str,
    flag: &'static str,
) -> Result<String, UsageError> {
    single(given, flag)?.ok_or(UsageError::Missing { command, flag })
}

fn seconds(given: &[Given], flag: &'static str) -> Result<Option<Duration>, UsageError> {
    let Some(value) = single(given, flag)? else {
        return Ok(None);
    };
    match value.parse::<u64>() {
        Ok(seconds) if seconds > 0 => Ok(Some(Duration::from_secs(seconds))),
        _ => Err(UsageError::Invalid {
            flag,
            value,
            expected: "a whole number of seconds above zero",
        }),
    }
}

fn parse_run(given: Vec<Given>) -> Result<Command, UsageError> {
    let permission_mode = match single(&given, PERMISSION_MODE)? {
        None => PermissionMode::default(),
        Some(value) => PermissionMode::parse(&value).ok_or(UsageError::Invalid {
            flag: PERMISSION_MODE,
            value,
            expected: PermissionMode::NAMES,
        })?,
    };
    let workspace = PathBuf::from(required(&given, RUN, WORKSPACE)?);
    let args = RunArgs {
        workspace,
        permission_mode,
        answers: single(&given, ANSWERS)?.map(PathBuf::from),
        model: single(&given, MODEL)?,
        agent: single(&given, AGENT)?,
        data_root: single(&given, DATA_ROOT)?.map(PathBuf::from),
        sync_timeout: seconds(&given, SYNC_TIMEOUT)?.unwrap_or(DEFAULT_SYNC_TIMEOUT),
        timeout: seconds(&given, TIMEOUT)?,
        prompts: given
            .into_iter()
            .filter_map(|entry| match (entry.flag, entry.value) {
                (PROMPT, Some(text)) => Some(PromptSource::Text(text)),
                (PROMPT_FILE, Some(path)) => Some(PromptSource::File(PathBuf::from(path))),
                _ => None,
            })
            .collect(),
    };
    if args.prompts.is_empty() {
        return Err(UsageError::Missing {
            command: RUN,
            flag: PROMPT,
        });
    }
    Ok(Command::Run(Box::new(args)))
}

fn parse_login(given: &[Given]) -> Result<Command, UsageError> {
    if !given.iter().any(|entry| entry.flag == STDIN) {
        return Err(UsageError::Missing {
            command: LOGIN,
            flag: STDIN,
        });
    }
    Ok(Command::Login(LoginArgs {
        instance: required(given, LOGIN, INSTANCE)?,
        fingerprint: single(given, FINGERPRINT)?.map(KeyFingerprint::new),
        data_root: single(given, DATA_ROOT)?.map(PathBuf::from),
    }))
}

fn read(path: &Path) -> Result<String, UsageError> {
    std::fs::read_to_string(path).map_err(|source| UsageError::Unreadable {
        path: path.to_path_buf(),
        source,
    })
}

impl RunArgs {
    pub fn load(self) -> Result<RunRequest, UsageError> {
        let workspace =
            std::path::absolute(&self.workspace).map_err(|source| UsageError::Unreadable {
                path: self.workspace.clone(),
                source,
            })?;
        if !workspace.is_dir() {
            return Err(UsageError::NotADirectory { path: workspace });
        }
        let answers = match &self.answers {
            None => Answers::default(),
            Some(path) => Answers::parse(&read(path)?).map_err(|source| UsageError::Answers {
                path: path.clone(),
                source,
            })?,
        };
        let prompts = self
            .prompts
            .into_iter()
            .map(|prompt| match prompt {
                PromptSource::Text(text) => Ok(text),
                PromptSource::File(path) => {
                    read(&path).map(|text| text.trim_end_matches(['\r', '\n']).to_owned())
                },
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RunRequest {
            prompts,
            workspace,
            permission_mode: self.permission_mode,
            answers,
            model: self.model,
            agent: self.agent,
            data_root: self.data_root,
            sync_timeout: self.sync_timeout,
            timeout: self.timeout,
        })
    }
}
