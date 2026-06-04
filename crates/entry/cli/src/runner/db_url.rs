//! Classifies how each command relates to a directly supplied `--database-url`.
//!
//! The flag bypasses profile resolution, but only a subset of commands can run
//! against a bare connection. [`DbUrlRouting`] lets [`super::run`] decide the
//! routing before any connection is dialed, so profile-establishing commands
//! keep their wizard path and unsupported commands fail fast.

use super::args::Commands;
use crate::commands::{admin, cloud, core, infrastructure};

pub(super) enum DbUrlRouting {
    /// Dispatchable against the supplied connection alone (the
    /// `execute_with_db` path), with no profile.
    Direct,
    /// Establishes its own context (setup wizard, bootstrap, session); the flag
    /// is ignored so the command runs its normal path.
    ProfileDriven,
    /// Cannot run without full profile bootstrap; rejected before connecting.
    Unsupported,
}

impl Commands {
    /// The [`DbUrlRouting::Direct`] set must stay in lockstep with the
    /// `execute_with_db` arms each domain exposes — those are the only commands
    /// `run_with_database_url` can serve.
    pub(super) const fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Admin(
                admin::AdminCommands::Setup(_)
                | admin::AdminCommands::Bootstrap(_)
                | admin::AdminCommands::Session(_),
            ) => DbUrlRouting::ProfileDriven,

            Self::Core(core::CoreCommands::Content(_) | core::CoreCommands::Files(_))
            | Self::Infra(
                infrastructure::InfraCommands::Db(_) | infrastructure::InfraCommands::Logs(_),
            )
            | Self::Admin(admin::AdminCommands::Users(_))
            | Self::Analytics(_)
            | Self::Cloud(cloud::CloudCommands::Db(_)) => DbUrlRouting::Direct,

            _ => DbUrlRouting::Unsupported,
        }
    }
}
