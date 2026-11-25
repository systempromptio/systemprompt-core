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

use super::factories::{
    AudienceResolverFactory, DiagnosticFactory, FactoryKind, GatewayGuardFactory, PolicyFactory,
    ReadinessFactory, RunnerServiceFactory, SecretReferenceFactory, SubjectProviderFactory,
};
use super::sink::ContributionSink;

impl ContributionSink {
    #[must_use]
    pub fn subject_providers(&self) -> &[SubjectProviderFactory] {
        &self.subject_providers
    }

    pub fn audience_resolvers(&self) -> &[AudienceResolverFactory] {
        &self.audience_resolvers
    }

    #[must_use]
    pub fn policies(&self) -> &[PolicyFactory] {
        &self.policies
    }

    #[must_use]
    pub fn runner_services(&self) -> &[RunnerServiceFactory] {
        &self.runner_services
    }

    #[must_use]
    pub fn readiness(&self) -> &[ReadinessFactory] {
        &self.readiness
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[DiagnosticFactory] {
        &self.diagnostics
    }

    #[must_use]
    pub fn gateway_guards(&self) -> &[GatewayGuardFactory] {
        &self.gateway_guards
    }

    #[must_use]
    pub fn declared_secrets(&self) -> &[SecretPattern] {
        &self.declared_secrets
    }

    #[must_use]
    pub fn declared_conferrals(&self) -> &[crate::reserved::InstallConferral] {
        &self.conferrals
    }

    #[must_use]
    pub fn declared_token_audiences(&self) -> &[systemprompt_traits::TokenAudienceGrant] {
        &self.token_audiences
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
            admin_bootstrap: _,
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
            conferrals: _,
            token_audiences: _,
            secret_references,
            slots: _,
            late_cells: _,
            domain_configs: _,
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
