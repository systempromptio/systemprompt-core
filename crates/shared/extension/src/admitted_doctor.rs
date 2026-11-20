//! Every admitted member's `cloud doctor` checks, stamped with their owners
//! in composition order.
//!
//! The view is open: it holds check values, which read only what the doctor
//! hands them (`DoctorContext`), and is built from the admitted composition
//! without running any member code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::DoctorCheck;

use crate::admitted::BuiltState;

/// One admitted doctor check with its owner.
#[derive(Clone)]
pub struct OwnedDoctorCheck {
    pub owner: ExtensionId,
    pub check: Arc<dyn DoctorCheck>,
}

impl std::fmt::Debug for OwnedDoctorCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedDoctorCheck")
            .field("owner", &self.owner)
            .field("check", &self.check.name())
            .finish()
    }
}

/// The doctor checks of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedDoctorChecks {
    entries: Vec<OwnedDoctorCheck>,
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
                            check: Arc::clone(check),
                        })
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[OwnedDoctorCheck] {
        &self.entries
    }
}
