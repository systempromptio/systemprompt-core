//! A member's admitted standing: core (a member of the reference
//! composition) or install. Slots order and own their entries by it, and the
//! installer records it with each table owner and purge record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandingTag {
    Core,
    Install,
}

impl StandingTag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Install => "install",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "core" => Some(Self::Core),
            "install" => Some(Self::Install),
            _ => None,
        }
    }
}
