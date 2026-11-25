//! The API composition fixture: an [`Admitted`] handoff of probe members and
//! a [`Kernel`] resolved over it.
//!
//! A probe is a test member that contributes through the same
//! `Contributions` surface a real member uses — a gateway guard, a slot
//! entry, a config section — so a test exercises what the kernel builds from
//! an admitted composition instead of a hand-seeded state. Gateway routes are
//! privileged shapes a probe cannot claim; a test that needs one passes them
//! in [`ComposedParts::gateway_routes`].
//!
//! [`gateway_probe_admitted`] adds the `gateway` member under core standing:
//! its slots build the engine values `GatewayEngines::new` reads, so a test
//! that builds the engines resolves over it ([`gateway_resolved_kernel`]).

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedMember, AdmittedParts};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_extension::{
    Admitted, ContributionError, Contributions, Extension, ExtensionMetadata, StandingTag,
};
use systemprompt_runtime::{Kernel, KernelError};
use systemprompt_traits::SecretPattern;

type Contribute = Box<dyn Fn(&mut Contributions<'_>) + Send + Sync>;

pub struct Probe {
    id: &'static str,
    contribute: Contribute,
    secret_grants: Vec<SecretPattern>,
}

impl Probe {
    pub fn new(
        id: &'static str,
        contribute: impl Fn(&mut Contributions<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            contribute: Box::new(contribute),
            secret_grants: Vec::new(),
        }
    }

    #[must_use]
    pub fn granted(mut self, grants: &[&str]) -> Self {
        self.secret_grants = grants
            .iter()
            .map(|grant| SecretPattern::parse(grant).expect("a valid secret grant"))
            .collect();
        self
    }
}

impl Extension for Probe {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        (self.contribute)(c);
        Ok(())
    }
}

#[derive(Default)]
pub struct ComposedParts {
    pub probes: Vec<Probe>,
    pub gateway_routes: Vec<AdmittedGatewayRoute>,
}

pub fn probe_admitted(parts: ComposedParts) -> Admitted {
    admitted_over(Vec::new(), parts)
}

// The members a child host admits (`host_admitted`) under core standing,
// with the probes beside them under install standing.
pub fn host_probe_admitted(probes: Vec<Probe>) -> Admitted {
    let mut core = systemprompt_test_migrate::core_members();
    core.push(Box::new(
        systemprompt_documents::extension::DocumentsExtension,
    ));
    let members = core
        .iter()
        .map(|ext| {
            systemprompt_test_migrate::admitted_member(ext.as_ref(), StandingTag::Core)
                .unwrap_or_else(|e| panic!("{} contributes: {e}", ext.metadata().id))
        })
        .collect();
    admitted_over(
        members,
        ComposedParts {
            probes,
            ..ComposedParts::default()
        },
    )
}

pub fn authz_probe_admitted(parts: ComposedParts) -> Admitted {
    let authz = systemprompt_test_migrate::admitted_member(
        &systemprompt_governance::authz::AuthzExtension,
        StandingTag::Core,
    )
    .expect("the authz member contributes");
    admitted_over(vec![authz], parts)
}

pub fn gateway_probe_admitted(parts: ComposedParts) -> Admitted {
    admitted_over(vec![gateway_member()], parts)
}

fn gateway_member() -> AdmittedMember {
    systemprompt_test_migrate::admitted_member(
        &systemprompt_gateway::extension::GatewayExtension,
        StandingTag::Core,
    )
    .expect("the gateway member contributes")
}

fn admitted_over(mut members: Vec<AdmittedMember>, parts: ComposedParts) -> Admitted {
    members.extend(parts.probes.iter().map(|probe| {
        let mut member = systemprompt_test_migrate::admitted_member(probe, StandingTag::Install)
            .expect("a probe contributes");
        member.secret_grants.clone_from(&probe.secret_grants);
        member
    }));
    Admitted::__from_facade(AdmittedParts {
        withheld: Vec::new(),
        members,
        gateway_routes: parts.gateway_routes,
        ..AdmittedParts::default()
    })
}

pub fn resolved_kernel(
    pool: &DbPool,
    admitted: &Admitted,
    seed: Dependencies,
) -> Result<Kernel, KernelError> {
    let mut kernel = crate::app_context::fixture_kernel(pool, seed);
    kernel.resolve(admitted)?;
    Ok(kernel)
}

pub fn offline_resolved_kernel(admitted: &Admitted, seed: Dependencies) -> Kernel {
    let pool: DbPool = Arc::new(systemprompt_database::Database::from_pools(
        crate::db::lazy_pg_pool(),
        None,
    ));
    resolved_kernel(&pool, admitted, seed).expect("the kernel resolves the probe composition")
}

pub fn gateway_resolved_kernel() -> Kernel {
    offline_resolved_kernel(
        &gateway_probe_admitted(ComposedParts::default()),
        gateway_kernel_seed(),
    )
}

// The two sealed seeds the gateway member's engines provide reads on every
// host: the connector token cache and the provider-key lookup over the test
// bootstrap's secrets.
pub fn gateway_kernel_seed() -> Dependencies {
    let secrets: std::sync::Arc<dyn systemprompt_traits::SecretLookup> =
        std::sync::Arc::<systemprompt_config::SecretsService>::clone(
            crate::bootstrap::ensure_test_bootstrap().boot.secrets(),
        );
    Dependencies::new()
        .with_sealed(std::sync::Arc::<
            systemprompt_security::credential::TokenCache,
        >::default())
        .with_sealed(secrets)
}

pub fn empty_resolved_kernel() -> Kernel {
    offline_resolved_kernel(
        &probe_admitted(ComposedParts::default()),
        Dependencies::new(),
    )
}
