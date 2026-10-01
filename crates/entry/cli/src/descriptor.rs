//! Per-command bootstrap descriptor: which of profile/secrets/paths a command
//! needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Bootstrap flags for one command.
///
/// Model discovery (installing the services registry through boot-time
/// discovery) is set only for the server; every other command loads the YAML
/// catalog as authored and must not reach for the network.
///
/// `is_destructive` is never set by hand: it is projected from the command's
/// exhaustive [`DataImpact`] classification, so a command cannot be added
/// without deciding whether it destroys data.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandDescriptor {
    flags: u16,
}

impl CommandDescriptor {
    const FLAG_PROFILE: u16 = 0b0000_0000_0001;
    const FLAG_SECRETS: u16 = 0b0000_0000_0010;
    const FLAG_PATHS: u16 = 0b0000_0000_0100;
    const FLAG_DATABASE: u16 = 0b0000_0000_1000;
    const FLAG_REMOTE_ELIGIBLE: u16 = 0b0000_0001_0000;
    const FLAG_SKIP_VALIDATION: u16 = 0b0000_0010_0000;
    const FLAG_READ_ONLY: u16 = 0b0000_0100_0000;
    const FLAG_MODEL_DISCOVERY: u16 = 0b0000_1000_0000;
    const FLAG_DESTRUCTIVE: u16 = 0b0001_0000_0000;

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

    pub const fn discovers_models(&self) -> bool {
        self.flags & Self::FLAG_MODEL_DISCOVERY != 0
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

    pub const fn with_model_discovery(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_MODEL_DISCOVERY,
        }
    }

    pub const fn is_destructive(&self) -> bool {
        self.flags & Self::FLAG_DESTRUCTIVE != 0
    }

    pub const fn data_impact(&self) -> DataImpact {
        if self.is_destructive() {
            DataImpact::Destructive
        } else {
            DataImpact::Preserving
        }
    }

    pub const fn with_data_impact(self, impact: DataImpact) -> Self {
        match impact {
            DataImpact::Destructive => Self {
                flags: self.flags | Self::FLAG_DESTRUCTIVE,
            },
            DataImpact::Preserving => self,
        }
    }
}

/// How a command treats the data behind the profile it resolves.
///
/// `Destructive` commands delete rows, rewrite schema, run jobs or change
/// privilege. They refuse a cloud profile that arrived implicitly (stored
/// session or directory discovery), and a failed remote route never falls
/// back to direct database access for them. Every command enum classifies
/// its variants in an exhaustive `match` with no wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataImpact {
    Preserving,
    Destructive,
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
