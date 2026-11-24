//! Every admitted member's `cloud doctor` checks, stamped with their owners
//! in composition order.
//!
//! The view is open: it holds check values, which read only what the doctor
//! hands them (`DoctorContext`: secret presence and read-only probes), and
//! is built from the admitted composition without running any member code.
//! Core and install members' checks are both admitted and are run by
//! [`AdmittedDoctorChecks::run`] against one context, so no standing sees
//! more than another. It also carries the first core member's
//! `DoctorProbeSource` (with the `sqlx` feature): the doctor opens core
//! checks' probes through it, and an install member's source is never read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{DoctorCheck, DoctorContext, DoctorOutcome};

use crate::admitted::BuiltState;
use crate::contribute::StandingTag;

/// One admitted doctor check with its owner.
#[derive(Clone)]
pub struct OwnedDoctorCheck {
    pub owner: ExtensionId,
    pub standing: StandingTag,
    pub check: Arc<dyn DoctorCheck>,
}

impl std::fmt::Debug for OwnedDoctorCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedDoctorCheck")
            .field("owner", &self.owner)
            .field("standing", &self.standing)
            .field("check", &self.check.name())
            .finish()
    }
}

/// The doctor checks of an admitted composition.
#[derive(Clone, Default)]
pub struct AdmittedDoctorChecks {
    entries: Vec<OwnedDoctorCheck>,
    #[cfg(feature = "sqlx")]
    probe_source: Option<Arc<dyn systemprompt_provider_contracts::DoctorProbeSource>>,
}

impl std::fmt::Debug for AdmittedDoctorChecks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmittedDoctorChecks")
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

impl AdmittedDoctorChecks {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        Self {
            entries: built
                .members()
                .flat_map(|member| {
                    member
                        .sink
                        .doctor_checks()
                        .iter()
                        .map(|check| OwnedDoctorCheck {
                            owner: member.owner.clone(),
                            standing: member.standing,
                            check: Arc::clone(check),
                        })
                })
                .collect(),
            #[cfg(feature = "sqlx")]
            probe_source: built
                .members()
                .filter(|member| member.standing == StandingTag::Core)
                .find_map(|member| member.sink.doctor_probe_sources().first().cloned()),
        }
    }

    #[cfg(feature = "sqlx")]
    #[must_use]
    pub fn probe_source(
        &self,
    ) -> Option<&Arc<dyn systemprompt_provider_contracts::DoctorProbeSource>> {
        self.probe_source.as_ref()
    }

    #[must_use]
    pub fn entries(&self) -> &[OwnedDoctorCheck] {
        &self.entries
    }

    pub async fn run(&self, ctx: DoctorContext<'_>) -> Vec<(&OwnedDoctorCheck, DoctorOutcome)> {
        let mut outcomes = Vec::with_capacity(self.entries.len());
        for owned in &self.entries {
            outcomes.push((owned, owned.check.check(ctx).await));
        }
        outcomes
    }
}
