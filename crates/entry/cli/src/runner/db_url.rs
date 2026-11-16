//! Classifies how each command relates to a directly supplied `--database-url`.
//!
//! The flag bypasses profile resolution, but only a subset of commands can run
//! against a bare connection. [`DbUrlRouting`] lets [`super::run`] decide the
//! routing before any connection is dialed, so profile-establishing commands
//! keep their wizard path and unsupported commands fail fast. Registered
//! command groups declare theirs through
//! [`CliCommandGroup::db_url_routing`](crate::kernel::CliCommandGroup::db_url_routing).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::args::Commands;
use crate::commands::{admin, core, infrastructure};
use crate::descriptor::DbUrlRouting;

impl admin::AdminCommands {
    const fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Setup(_)
            | Self::Bootstrap(_)
            | Self::Session(_)
            | Self::Config(admin::config::ConfigCommands::Apply(_)) => DbUrlRouting::ProfileDriven,

            Self::Users(_) => DbUrlRouting::Direct,

            Self::Config(admin::config::ConfigCommands::Migrate(args))
                if args.services.is_some() =>
            {
                DbUrlRouting::Direct
            },

            _ => DbUrlRouting::Unsupported,
        }
    }
}

impl Commands {
    pub(super) const fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Admin(cmd) => cmd.db_url_routing(),

            Self::Core(core::CoreCommands::Files(_))
            | Self::Infra(
                infrastructure::InfraCommands::Db(_)
                | infrastructure::InfraCommands::Logs(_)
                | infrastructure::InfraCommands::Jobs(infrastructure::jobs::JobsCommands::History(_)),
            ) => DbUrlRouting::Direct,

            _ => DbUrlRouting::Unsupported,
        }
    }
}
