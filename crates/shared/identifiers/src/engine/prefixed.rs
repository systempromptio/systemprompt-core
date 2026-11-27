//! Minted identifiers that sort in creation order.
//!
//! A project is one worktree on disk; a session one conversation, possibly the
//! child of another; a message and a part belong to a session; a job is a
//! background shell job whose id the model uses to read or stop it;
//! a permission request and a question request are what the engine puts to the
//! user while a tool waits.
//!
//! Each type carries its prefix, mints from an [`IdClock`] with `mint()`,
//! validates with `parse()`
//! (also behind `FromStr`, `TryFrom<String>` and `Deserialize`) and serialises
//! as its plain string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use crate::engine::clock::{IdClock, created_ms, validate};
use crate::engine::error::IdError;

macro_rules! define_id {
    ($name:ident, $prefix:literal, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            pub fn mint(clock: &IdClock) -> Self {
                Self(clock.mint($prefix))
            }

            pub fn parse(value: &str) -> Result<Self, IdError> {
                validate($prefix, $kind, value)?;
                Ok(Self(value.to_owned()))
            }

            pub const fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn created_at_ms(&self) -> i64 {
                created_ms(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(value: &str) -> Result<Self, IdError> {
                Self::parse(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(value: String) -> Result<Self, IdError> {
                validate($prefix, $kind, &value)?;
                Ok(Self(value))
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

define_id!(ProjectId, "prj", "project");
define_id!(SessionId, "ses", "session");
define_id!(MessageId, "msg", "message");
define_id!(PartId, "prt", "part");
define_id!(JobId, "job", "job");
define_id!(PermissionId, "per", "permission");
define_id!(QuestionId, "qst", "question");
