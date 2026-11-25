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
//! - [`InstallSeed::SystemAdmin`] — the system admin's identity, the owner an
//!   install attributes managed inventory to; an identifier, no credential.
//! - [`InstallSeed::ConfigView`] — the runtime configuration without the
//!   database URLs and the signing key path (`ConfigView`): public and internal
//!   URLs, the JWT issuer and lifetimes, rate limits, paths.
//! - [`InstallSeed::ProfileView`] — the profile's name, target, runtime
//!   environment, judge mode, cloud tenant id, OTLP target without its
//!   credentials, and `paths.bin` (`ProfileView`).
//! - [`InstallSeed::ServicesView`] — the active services root, the loaded
//!   services config and catalog, and the bundle source list without source
//!   credentials (`InstallServicesView`); no handle that writes or reloads.
//!
//! No route set's state reads any row: a route set reads what its member
//! built in a provide fn. An install's secrets are not a row: the store is
//! withheld, and an install reads only the secret names it declared
//! (`Contributions::secret`), through `Resolved::secrets`. What an install
//! does with a sealed key it reaches only through a conferred capability
//! (`super::INSTALL_CONFERRALS`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{TypeId, type_name};
use std::sync::Arc;

use systemprompt_manifest::ConfigView;
use systemprompt_manifest::profile::ProfileView;
use systemprompt_manifest::services::{InstallServicesView, SystemAdmin};

/// One row of [`INSTALL_SEEDS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallSeed {
    SystemAdmin,
    ConfigView,
    ProfileView,
    ServicesView,
}

pub const INSTALL_SEEDS: &[InstallSeed] = &[
    InstallSeed::SystemAdmin,
    InstallSeed::ConfigView,
    InstallSeed::ProfileView,
    InstallSeed::ServicesView,
];

impl InstallSeed {
    #[must_use]
    pub const fn type_id(self) -> TypeId {
        match self {
            Self::SystemAdmin => TypeId::of::<SystemAdmin>(),
            Self::ConfigView => TypeId::of::<Arc<ConfigView>>(),
            Self::ProfileView => TypeId::of::<Arc<ProfileView>>(),
            Self::ServicesView => TypeId::of::<InstallServicesView>(),
        }
    }

    #[must_use]
    pub fn type_name(self) -> &'static str {
        match self {
            Self::SystemAdmin => type_name::<SystemAdmin>(),
            Self::ConfigView => type_name::<Arc<ConfigView>>(),
            Self::ProfileView => type_name::<Arc<ProfileView>>(),
            Self::ServicesView => type_name::<InstallServicesView>(),
        }
    }
}

#[must_use]
pub fn install_reads(type_id: TypeId) -> bool {
    INSTALL_SEEDS.iter().any(|seed| seed.type_id() == type_id)
}
