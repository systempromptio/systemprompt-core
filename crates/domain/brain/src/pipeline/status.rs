//! The status stage.
//!
//! A new fact's status, stored importance and review date. `Always`
//! confirms; `Never` leaves an extracted fact `Proposed` and an entered
//! fact in the status its author chose. `ProjectPolicy` names a per-project
//! policy no project yet declares, so it confirms nothing. The base
//! importance comes from the first extension model that scores the
//! candidate, else from core's formula.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use systemprompt_extension::brain::ImportanceModel;
use systemprompt_manifest::profile::{AutoConfirm, AutoConfirmPolicy};
use systemprompt_models::brain::{
    EnteredStatus, FactCandidate, FactSource, FactStatus, ImportanceConfig, KindDecl,
};

use super::normalise::{EventPayload, Normalised};
use crate::defaults::FormulaImportance;

/// A new fact's lifecycle settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lifecycle {
    pub status: FactStatus,
    pub importance_base: f64,
    pub review_after: DateTime<Utc>,
}

#[must_use]
pub fn status_for(
    policy: &AutoConfirmPolicy,
    event: &Normalised,
    candidate: &FactCandidate,
) -> FactStatus {
    let rule = match candidate.source {
        FactSource::Document { .. } | FactSource::Comment { .. } => policy.document,
        _ => policy.others,
    };
    if rule == AutoConfirm::Always {
        return FactStatus::Confirmed;
    }
    match &event.payload {
        EventPayload::Entered(entered) => match entered.status {
            EnteredStatus::Candidate => FactStatus::Candidate,
            EnteredStatus::Proposed => FactStatus::Proposed,
        },
        EventPayload::Revision(_) | EventPayload::Comment(_) => FactStatus::Proposed,
    }
}

#[must_use]
pub fn importance(
    models: &[Arc<dyn ImportanceModel>],
    candidate: &FactCandidate,
    config: &ImportanceConfig,
    confirmed: bool,
) -> f64 {
    models
        .iter()
        .find_map(|model| model.base(candidate, config))
        .unwrap_or_else(|| FormulaImportance::base_with(candidate, config, confirmed))
}

#[must_use]
pub fn review_after(kind: &KindDecl, default_days: u32, now: DateTime<Utc>) -> DateTime<Utc> {
    let days = kind.review_after_days.unwrap_or(default_days);
    now + Duration::days(i64::from(days))
}
