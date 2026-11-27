//! The headless entry: one session run from the command line, and a
//! non-interactive sign-in.
//!
//! [`run`] starts the same composition the window starts (accounts, the
//! session store, governance installed as the engine's Governor, the registered
//! extensions' engine halves, the live sync) without a window, waits for the
//! active instance to be applied, drives one session through its prompts and
//! writes every store change and engine event as one JSON line
//! ([`RunEvent`]) to the writer it is given. Permission requests are answered
//! by the [`PermissionMode`] and the scripted [`Answers`]; nothing waits for a
//! person. [`login`] signs in with a personal access token read from standard
//! input and pins the manifest key the instance serves.
//!
//! [`parse`] turns an argument list into a [`Command`], and [`execute`] is
//! the whole dispatch a binary calls: it returns the [`ExitStatus`] the
//! process ends with. The extensions' background services are not started: a
//! headless run reaches the instance only for sign-in, sync, model calls,
//! verdicts and governance events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod args;
mod compose;
mod control;
mod dispatch;
mod event;
mod exit;
mod host;
mod login;
mod permission;
mod pump;
mod run;
mod settings;
mod slot;
mod stop;
mod usage;

pub use args::{Command, LoginArgs, PromptSource, RunArgs, parse};
pub use compose::{Composition, StartupError};
pub use control::{Control, ControlDelivery, ControlFeed};
pub use dispatch::{Streams, execute};
pub use event::{EventWriter, Feed, RunEvent};
pub use exit::ExitStatus;
pub use host::{DataRoot, HeadlessHost};
pub use login::{LoginFailure, LoginOutcome, LoginReport, LoginRequest, login, read_token};
pub use permission::{
    AnswerSource, Answers, Decider, PermissionMode, QuestionAnswers, UnusedAnswers,
};
pub use run::{DEFAULT_SYNC_TIMEOUT, RunReport, RunRequest, run, run_controlled};
pub use settings::MemorySettings;
pub use slot::GovernorSlot;
pub use usage::{USAGE, UsageError};
