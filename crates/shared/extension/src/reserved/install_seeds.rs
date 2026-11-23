//! The closed table of host-sealed seeds a member with install standing may
//! read.
//!
//! The kernel resolves an install member's provide fns, factories, slot
//! builds and governance factories with its standing: every host-sealed seed
//! whose type is not a row here, and every value another member provided
//! sealed, is withheld from them with the `MissingDependency` an absent type
//! gets, so the provide fails typed at admission. A core member is not
//! narrowed. Each row names what it lets an install do:
//!
//! - [`InstallSeed::Config`] — the resolved runtime configuration: public and
//!   internal URLs, the JWT issuer and lifetimes, rate limits, paths, the
//!   database type. It carries the database URL the install's open database
//!   handle already connects with, and the optional GitHub token.
//! - [`InstallSeed::SystemAdmin`] — the system admin's identity, the owner an
//!   install attributes managed inventory to; an identifier, no credential.
//!
//! An install's secrets are not a row: the store is withheld, and an install
//! reads only the secret names it declared (`Contributions::secret`), through
//! `Resolved::secrets`, each refused when a core member, the profile or the
//! platform references it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{TypeId, type_name};
use std::sync::Arc;

use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;

/// One row of [`INSTALL_SEEDS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallSeed {
    Config,
    SystemAdmin,
}

pub const INSTALL_SEEDS: &[InstallSeed] = &[InstallSeed::Config, InstallSeed::SystemAdmin];

impl InstallSeed {
    #[must_use]
    pub const fn type_id(self) -> TypeId {
        match self {
            Self::Config => TypeId::of::<Arc<Config>>(),
            Self::SystemAdmin => TypeId::of::<SystemAdmin>(),
        }
    }

    #[must_use]
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Config => type_name::<Arc<Config>>(),
            Self::SystemAdmin => type_name::<SystemAdmin>(),
        }
    }
}

#[must_use]
pub fn install_reads(type_id: TypeId) -> bool {
    INSTALL_SEEDS.iter().any(|seed| seed.type_id() == type_id)
}
