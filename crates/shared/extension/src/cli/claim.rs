//! The routing and data-impact claims a command may make.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A property of a command's descriptor that relaxes how the CLI runner
/// treats the database behind the resolved profile.
///
/// A command that touches the database and claims nothing routes remotely
/// whenever the profile is a cloud profile, never falls back to direct
/// database access, is treated as destroying data (it refuses a cloud
/// profile that arrived implicitly) and refuses a bare `--database-url`.
/// Each claim relaxes one of these:
/// `ReadOnlyRouting` lets a remote-eligible command fall back to local
/// access with a warning; `LocalDatabase` runs it on this machine against
/// the profile's own database; `PreservesData` declares that it destroys
/// nothing; `DirectDatabase` runs it on a bare `--database-url` with no
/// profile. The first three are projected from the descriptor a parsed
/// command resolves; `DirectDatabase` is projected from the declaration
/// alone. A declaration lists the claims its commands make, the composition
/// and the CLI registry refuse a claim the reserved CLI path's row does not
/// confer, and the registry refuses a resolved descriptor that makes a claim
/// its declaration does not list. An install command holds no claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandClaim {
    ReadOnlyRouting,
    LocalDatabase,
    PreservesData,
    DirectDatabase,
}

impl CommandClaim {
    pub const ALL: [Self; 4] = [
        Self::ReadOnlyRouting,
        Self::LocalDatabase,
        Self::PreservesData,
        Self::DirectDatabase,
    ];
}

impl std::fmt::Display for CommandClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ReadOnlyRouting => "read-only-routing",
            Self::LocalDatabase => "local-database",
            Self::PreservesData => "preserves-data",
            Self::DirectDatabase => "direct-database",
        })
    }
}
