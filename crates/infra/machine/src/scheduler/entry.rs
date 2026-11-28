//! The typed description of a scheduler entry and what the scheduler answers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::Path;

use super::error::{EntryError, SchedulerError};
use crate::quote;

const ID_LIMIT: usize = 120;

const INTERVAL_LIMIT_MINUTES: u32 = 31 * 24 * 60;

/// The name an entry is registered under: the task name, the launchd label
/// and the unit base name. A product derives it from its brand so two brands
/// and several channels register side by side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(String);

impl EntryId {
    pub fn parse(value: &str) -> Result<Self, EntryError> {
        let plain = value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        let starts_plain = value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric());
        if !plain || !starts_plain || value.len() > ID_LIMIT {
            return Err(EntryError::Id {
                value: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A whole number of minutes between runs, from one minute to 31 days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Interval {
    minutes: u32,
}

impl Interval {
    pub const fn minutes(minutes: u32) -> Result<Self, EntryError> {
        if minutes == 0 || minutes > INTERVAL_LIMIT_MINUTES {
            return Err(EntryError::Interval { minutes });
        }
        Ok(Self { minutes })
    }

    #[must_use]
    pub const fn as_minutes(self) -> u32 {
        self.minutes
    }

    #[must_use]
    pub const fn as_seconds(self) -> u32 {
        self.minutes * 60
    }
}

/// When the system starts the program: at each login of the user, at login
/// and then at every interval, or only when asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    AtLogin,
    Every(Interval),
    OnDemand,
}

/// One program the system starts for the current user, without elevation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerEntry {
    id: EntryId,
    description: String,
    program: String,
    arguments: Vec<String>,
    trigger: Trigger,
}

impl SchedulerEntry {
    pub fn new(
        id: EntryId,
        description: &str,
        program: &Path,
        arguments: Vec<String>,
        trigger: Trigger,
    ) -> Result<Self, EntryError> {
        if description.trim().is_empty() || !quote::is_plain(description) {
            return Err(EntryError::Description);
        }
        let Some(text) = quote::program_text(program) else {
            return Err(EntryError::Program {
                path: program.to_path_buf(),
            });
        };
        if let Some(index) = arguments.iter().position(|a| !quote::is_plain(a)) {
            return Err(EntryError::Argument { index });
        }
        Ok(Self {
            id,
            description: description.to_owned(),
            program: text.to_owned(),
            arguments,
            trigger,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &EntryId {
        &self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    #[must_use]
    pub const fn trigger(&self) -> Trigger {
        self.trigger
    }
}

/// Whether an entry is registered with the system's scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    Installed,
    NotInstalled,
}

/// How a registration ended. `Inactive` is an entry whose files are in place
/// on a machine with no running scheduler for the user (a container, a
/// session without a user manager): it starts once one exists.
#[derive(Debug)]
pub enum Created {
    Active,
    Inactive { cause: SchedulerError },
}

/// How a removal ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Removed,
    NotInstalled,
}
