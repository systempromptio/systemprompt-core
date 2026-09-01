//! Per-command bootstrap descriptor: which of profile/secrets/paths a command
//! needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandDescriptor {
    flags: u8,
}

impl CommandDescriptor {
    const FLAG_PROFILE: u8 = 0b0000_0001;
    const FLAG_SECRETS: u8 = 0b0000_0010;
    const FLAG_PATHS: u8 = 0b0000_0100;
    const FLAG_DATABASE: u8 = 0b0000_1000;
    const FLAG_REMOTE_ELIGIBLE: u8 = 0b0001_0000;
    const FLAG_SKIP_VALIDATION: u8 = 0b0010_0000;
    const FLAG_READ_ONLY: u8 = 0b0100_0000;

    pub const NONE: Self = Self { flags: 0 };

    pub const PROFILE_ONLY: Self = Self {
        flags: Self::FLAG_PROFILE,
    };

    pub const PROFILE_AND_SECRETS: Self = Self {
        flags: Self::FLAG_PROFILE | Self::FLAG_SECRETS,
    };

    pub const PROFILE_SECRETS_AND_PATHS: Self = Self {
        flags: Self::FLAG_PROFILE | Self::FLAG_SECRETS | Self::FLAG_PATHS,
    };

    pub const FULL: Self = Self {
        flags: Self::FLAG_PROFILE
            | Self::FLAG_SECRETS
            | Self::FLAG_PATHS
            | Self::FLAG_DATABASE
            | Self::FLAG_REMOTE_ELIGIBLE,
    };

    pub const fn profile(&self) -> bool {
        self.flags & Self::FLAG_PROFILE != 0
    }

    pub const fn secrets(&self) -> bool {
        self.flags & Self::FLAG_SECRETS != 0
    }

    pub const fn paths(&self) -> bool {
        self.flags & Self::FLAG_PATHS != 0
    }

    pub const fn database(&self) -> bool {
        self.flags & Self::FLAG_DATABASE != 0
    }

    // Why: read-only and mutating were one boolean; refusing a read because no
    // tenant session exists fails the caller for a reason they cannot act on.
    pub const fn routing_class(&self) -> RoutingClass {
        if self.flags & Self::FLAG_REMOTE_ELIGIBLE == 0 {
            RoutingClass::LocalOnly
        } else if self.flags & Self::FLAG_READ_ONLY != 0 {
            RoutingClass::ReadOnly
        } else {
            RoutingClass::Mutating
        }
    }

    pub const fn skip_validation(&self) -> bool {
        self.flags & Self::FLAG_SKIP_VALIDATION != 0
    }

    pub const fn with_remote_eligible(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_REMOTE_ELIGIBLE,
        }
    }

    pub const fn with_read_only(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_READ_ONLY,
        }
    }

    pub const fn with_skip_validation(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_SKIP_VALIDATION,
        }
    }
}

/// What a command is allowed to do when the active profile is a cloud profile.
///
/// `LocalOnly` is never routed remotely and runs against whatever the profile
/// resolves; `ReadOnly` prefers remote when a session is available and falls
/// back to local with a warning; `Mutating` must route remotely or fail loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingClass {
    LocalOnly,
    ReadOnly,
    Mutating,
}

pub trait DescribeCommand {
    fn descriptor(&self) -> CommandDescriptor;
}
