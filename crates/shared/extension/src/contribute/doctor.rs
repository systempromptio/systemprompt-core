//! The `cloud doctor` contributions: a member's checks and, for a core member
//! that owns the facts they read, the probe source the doctor opens over its
//! read-only pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::DoctorCheck;

use super::factories::DoctorCheckRef;
#[cfg(feature = "sqlx")]
use super::factories::DoctorProbeSourceRef;
use super::{ContributionSink, Contributions};

impl Contributions<'_> {
    pub fn doctor_check(&mut self, check: Arc<dyn DoctorCheck>) -> &mut Self {
        self.sink.doctor_checks.push(check);
        self
    }

    #[cfg(feature = "sqlx")]
    pub fn doctor_probe_source(&mut self, source: DoctorProbeSourceRef) -> &mut Self {
        self.sink.doctor_probe_sources.push(source);
        self
    }
}

impl ContributionSink {
    #[must_use]
    pub fn doctor_checks(&self) -> &[DoctorCheckRef] {
        &self.doctor_checks
    }

    #[cfg(feature = "sqlx")]
    #[must_use]
    pub fn doctor_probe_sources(&self) -> &[DoctorProbeSourceRef] {
        &self.doctor_probe_sources
    }
}
