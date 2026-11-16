//! The API composition fixture: an [`Admitted`] handoff of probe members and
//! a [`Kernel`] resolved over it.
//!
//! A probe is a test member that contributes through the same
//! `Contributions` surface a real member uses — a gateway guard, a slot
//! entry, a config section — so a test exercises what the kernel builds from
//! an admitted composition instead of a hand-seeded state. Gateway routes are
//! privileged shapes a probe cannot claim; a test that needs one passes them
//! in [`ComposedParts::gateway_routes`].

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::prelude::Dependencies;
use systemprompt_extension::{
    Admitted, ContributionError, Contributions, Extension, ExtensionMetadata, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::{Kernel, KernelError};

type Contribute = Box<dyn Fn(&mut Contributions<'_>) + Send + Sync>;

pub struct Probe {
    id: &'static str,
    contribute: Contribute,
}

impl Probe {
    pub fn new(
        id: &'static str,
        contribute: impl Fn(&mut Contributions<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            contribute: Box::new(contribute),
        }
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
    let members = parts
        .probes
        .iter()
        .map(|probe| {
            let owner = ExtensionId::new(probe.id);
            let mut sink = ContributionSink::default();
            probe
                .contribute(&mut Contributions::__new(owner.clone(), &mut sink))
                .expect("a probe contributes");
            AdmittedMember {
                owner,
                metadata: probe.metadata(),
                standing: StandingTag::Install,
                sink,
            }
        })
        .collect();
    Admitted::__from_facade(AdmittedParts {
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

pub fn empty_resolved_kernel() -> Kernel {
    offline_resolved_kernel(&probe_admitted(ComposedParts::default()), Dependencies::new())
}
