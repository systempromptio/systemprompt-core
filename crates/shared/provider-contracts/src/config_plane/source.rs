//! [`ChangeSource`]: the closed vocabulary naming where a config row's
//! current value came from.
//!
//! Four origins exist and nothing else: a file in the services tree
//! (`code:<file>`), a pinned remote bundle (`bundle:<name>`), a console edit
//! by a user (`console:<user>`) and a schema migration (`migration`). The text
//! form is what the change context hands Postgres and what the `source`
//! columns store; [`ChangeSource::from_str`] refuses anything outside it, so a
//! new origin is a change to this type rather than a new string at a call
//! site.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_identifiers::UserId;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChangeSource {
    Code(String),
    Bundle(String),
    Console(UserId),
    Migration,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChangeSourceError {
    #[error(
        "'{0}' is not a change source: expected code:<file>, bundle:<name>, console:<user> or \
         migration"
    )]
    Unknown(String),
    #[error("change source '{0}:' names no {0}")]
    Empty(&'static str),
}

impl ChangeSource {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Code(_) => "code",
            Self::Bundle(_) => "bundle",
            Self::Console(_) => "console",
            Self::Migration => "migration",
        }
    }
}

impl fmt::Display for ChangeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code(file) => write!(f, "code:{file}"),
            Self::Bundle(name) => write!(f, "bundle:{name}"),
            Self::Console(user) => write!(f, "console:{}", user.as_str()),
            Self::Migration => f.write_str("migration"),
        }
    }
}

impl FromStr for ChangeSource {
    type Err = ChangeSourceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == "migration" {
            return Ok(Self::Migration);
        }
        let Some((kind, rest)) = text.split_once(':') else {
            return Err(ChangeSourceError::Unknown(text.to_owned()));
        };
        let (kind, build): (&'static str, fn(&str) -> Self) = match kind {
            "code" => ("code", |rest| Self::Code(rest.to_owned())),
            "bundle" => ("bundle", |rest| Self::Bundle(rest.to_owned())),
            "console" => ("console", |rest| Self::Console(UserId::new(rest))),
            _ => return Err(ChangeSourceError::Unknown(text.to_owned())),
        };
        if rest.is_empty() {
            return Err(ChangeSourceError::Empty(kind));
        }
        Ok(build(rest))
    }
}

impl Serialize for ChangeSource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ChangeSource {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
