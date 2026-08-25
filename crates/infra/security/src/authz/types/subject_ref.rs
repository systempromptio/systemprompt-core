//! `SubjectRef`: tagged reference to the subject a rule binds to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DepartmentId, RoleId, UserId};

use super::kinds::RuleType;

/// Tagged-union reference to a rule subject.
///
/// Bundles the dimension discriminator and the typed id so they can never
/// drift apart, mirroring [`super::EntityRef`] on the entity side. The serde
/// tags are the resolver's `rule_type` vocabulary (`user`, `department`,
/// `role`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum SubjectRef {
    User(UserId),
    Department(DepartmentId),
    Role(RoleId),
}

impl SubjectRef {
    #[must_use]
    pub fn rule_type(&self) -> RuleType {
        match self {
            Self::User(_) => RuleType::USER,
            Self::Department(_) => RuleType::from("department"),
            Self::Role(_) => RuleType::ROLE,
        }
    }

    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::User(id) => id.as_str(),
            Self::Department(id) => id.as_str(),
            Self::Role(id) => id.as_str(),
        }
    }
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.rule_type(), self.value())
    }
}
