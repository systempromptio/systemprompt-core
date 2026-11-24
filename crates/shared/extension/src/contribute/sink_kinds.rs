//! A member's declared secret names and secret-reference factories, and the
//! walk that counts a sink's factories by [`FactoryKind`].
//!
//! [`ContributionSink::factory_count`] destructures every sink field, so a
//! field added to the sink is a compile error here until it is classified as
//! a factory kind or as plain data.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::SecretPattern;

use super::factories::{FactoryKind, SecretReferenceFactory};
use super::sink::ContributionSink;

impl ContributionSink {
    #[must_use]
    pub fn declared_secrets(&self) -> &[SecretPattern] {
        &self.declared_secrets
    }

    #[must_use]
    pub fn secret_references(&self) -> &[SecretReferenceFactory] {
        &self.secret_references
    }

    #[must_use]
    pub fn factory_count(&self, kind: FactoryKind) -> usize {
        let Self {
            schemas: _,
            retirements: _,
            migrations: _,
            seeds: _,
            cross_tables: _,
            user_purges: _,
            orphan_sweeps: _,
            owner_reassignments: _,
            doctor_checks: _,
            #[cfg(feature = "sqlx")]
                doctor_probe_sources: _,
            roles: _,
            assets: _,
            configs: _,
            config_planes: _,
            site_auth: _,
            provides: _,
            requires: _,
            depends_on: _,
            jobs: _,
            job_run_scopes: _,
            retention: _,
            projections: _,
            run_origins: _,
            runner_services,
            readiness,
            diagnostics,
            metrics: _,
            gateway_guards,
            subject_providers,
            audience_resolvers,
            policies,
            declared_secrets: _,
            secret_references,
            slots: _,
            #[cfg(feature = "router")]
                routes: _,
            #[cfg(feature = "router")]
                gateway_routes: _,
            #[cfg(feature = "cli")]
                commands: _,
            errors: _,
        } = self;
        match kind {
            FactoryKind::RunnerService => runner_services.len(),
            FactoryKind::Readiness => readiness.len(),
            FactoryKind::Diagnostic => diagnostics.len(),
            FactoryKind::GatewayGuard => gateway_guards.len(),
            FactoryKind::SubjectProvider => subject_providers.len(),
            FactoryKind::Policy => policies.len(),
            FactoryKind::AudienceResolver => audience_resolvers.len(),
            FactoryKind::SecretReferences => secret_references.len(),
        }
    }
}
