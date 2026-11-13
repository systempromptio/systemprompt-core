//! The extensions the profile leaves enabled, and the contributions the
//! scheduler may run because of it.
//!
//! Every job, retention purge, retention sweep and projection drain is
//! registered with the id of the extension that owns it. The scheduler runs a
//! registration only while that extension is enabled, using the registry's one
//! definition of "enabled" ([`ExtensionRegistry::enabled_extensions`]): an
//! extension listed in `profile.extensions.disabled` contributes nothing, and a
//! profile that disables a required extension or an enabled extension's
//! dependency is refused. A registration whose owner is not a linked
//! extension is not run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobRegistration, registered_jobs};

use crate::error::SchedulerResult;

/// The ids of the extensions the profile leaves enabled.
#[derive(Debug, Clone, Default)]
pub struct EnabledExtensions {
    ids: HashSet<String>,
}

impl EnabledExtensions {
    pub fn resolve(
        registry: &ExtensionRegistry,
        disabled: &[ExtensionId],
    ) -> SchedulerResult<Self> {
        let ids = registry
            .enabled_extensions(disabled)?
            .iter()
            .map(|ext| ext.id().to_owned())
            .collect();
        Ok(Self { ids })
    }

    pub fn of(app: &AppContext) -> SchedulerResult<Self> {
        Self::resolve(app.extension_registry(), &app.profile().extensions.disabled)
    }

    #[must_use]
    pub fn contains(&self, owner: &str) -> bool {
        self.ids.contains(owner)
    }

    pub fn registrations(&self) -> impl Iterator<Item = &'static JobRegistration> + '_ {
        registered_jobs().filter(|registration| self.contains(registration.owner))
    }

    pub fn jobs(&self) -> impl Iterator<Item = &'static dyn Job> + '_ {
        self.registrations().map(|registration| registration.job)
    }

    #[must_use]
    pub fn job(&self, name: &str) -> Option<&'static dyn Job> {
        self.jobs().find(|job| job.name() == name)
    }
}
