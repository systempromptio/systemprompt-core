//! How a command treats the data and the database behind its profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// How a command treats the data behind the profile it resolves.
///
/// `Destructive` commands delete rows, rewrite schema, run jobs or change
/// privilege. They refuse a cloud profile that arrived implicitly (stored
/// session or directory discovery), and a failed remote route never falls
/// back to direct database access for them. Every command enum classifies
/// its variants in an exhaustive `match` with no wildcard arm.
///
/// `Irreversible` is `Destructive` plus an operator confirmation: the runner
/// prompts before the command runs, and with no terminal the command refuses
/// unless `--yes` was passed. Such a command flattens
/// [`ConfirmArgs`](super::ConfirmArgs) so that `--yes` parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataImpact {
    Preserving,
    Destructive,
    Irreversible,
}

impl DataImpact {
    pub const fn destroys_data(self) -> bool {
        match self {
            Self::Preserving => false,
            Self::Destructive | Self::Irreversible => true,
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

/// How a command relates to a directly supplied `--database-url`.
///
/// The flag bypasses profile resolution, but only a subset of commands can run
/// against a bare connection: `Direct` commands do, `ProfileDriven` commands
/// keep their profile-establishing path and ignore it, and `Unsupported`
/// commands fail before any connection is dialed. A kernel command's routing
/// is projected from its arguments; a member command's is `Direct` exactly
/// when its declaration holds
/// [`CommandClaim::DirectDatabase`](super::CommandClaim::DirectDatabase) and
/// `Unsupported` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbUrlRouting {
    Direct,
    ProfileDriven,
    Unsupported,
}
