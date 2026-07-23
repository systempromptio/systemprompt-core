//! Classifies how each command relates to a directly supplied `--database-url`.
//!
//! The flag bypasses profile resolution, but only a subset of commands can run
//! against a bare connection. [`DbUrlRouting`] lets [`super::run`] decide the
//! routing before any connection is dialed, so profile-establishing commands
//! keep their wizard path and unsupported commands fail fast.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::args::Commands;
use crate::commands::{admin, cloud, core, infrastructure};

pub(super) enum DbUrlRouting {
    /// Dispatchable against the supplied connection alone, with no profile.
    Direct,
    /// Establishes its own context (setup wizard, bootstrap, session); the flag
    /// is ignored so the command runs its normal path.
    ProfileDriven,
    /// Cannot run without full profile bootstrap; rejected before connecting.
    Unsupported,
}

impl Commands {
    // Why: The [`DbUrlRouting::Direct`] set must stay in lockstep with the
    // database-scoped gates in each group's `execute` — those are the only
    // commands `run_with_database_url` can serve.
    pub(super) const fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Admin(
                admin::AdminCommands::Setup(_)
                | admin::AdminCommands::Bootstrap(_)
                | admin::AdminCommands::Session(_),
            ) => DbUrlRouting::ProfileDriven,

            Self::Core(core::CoreCommands::Content(_) | core::CoreCommands::Files(_))
            | Self::Infra(
                infrastructure::InfraCommands::Db(_)
                | infrastructure::InfraCommands::Logs(_)
                | infrastructure::InfraCommands::Jobs(
                    infrastructure::jobs::JobsCommands::List
                    | infrastructure::jobs::JobsCommands::Show(_)
                    | infrastructure::jobs::JobsCommands::History(_),
                ),
            )
            | Self::Admin(admin::AdminCommands::Users(_))
            | Self::Analytics(_)
            | Self::Cloud(cloud::CloudCommands::Db(_)) => DbUrlRouting::Direct,

            _ => DbUrlRouting::Unsupported,
        }
    }
}
