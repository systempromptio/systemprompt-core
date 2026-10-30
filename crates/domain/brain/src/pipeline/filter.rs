//! The filter stage: extension filters by ascending priority decide whether
//! an event is worth reading. The first `Gather` or `Ignore` wins and an
//! abstaining filter defers to the next; with no filter registered core
//! gathers nothing. A person-entered fact bypasses the filters, and a
//! pathway the profile does not capture is ignored before any filter runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::brain::{EventFilter, FilterVerdict};
use systemprompt_identifiers::FactKindId;
use systemprompt_manifest::profile::{BrainCaptureProfile, CaptureSource};
use systemprompt_models::brain::{BrainEnvelope, Pathway};

pub const CAPTURE_SOURCES: &str = "capture.sources";
pub const ENTERED_BYPASS: &str = "entered";
pub const ALL_ABSTAINED: &str = "abstained";

/// What the composite filter decided for one event. An empty `kinds`
/// gathers every declared kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    NoFilter,
    Gather {
        filter: String,
        kinds: Vec<FactKindId>,
    },
    Ignored {
        filter: String,
    },
}

impl FilterDecision {
    #[must_use]
    pub fn filter_name(&self) -> Option<&str> {
        match self {
            Self::NoFilter => None,
            Self::Gather { filter, .. } | Self::Ignored { filter } => Some(filter),
        }
    }
}

#[must_use]
pub fn decide(
    filters: &[Arc<dyn EventFilter>],
    capture: &BrainCaptureProfile,
    envelope: &BrainEnvelope,
) -> FilterDecision {
    if !capture.sources.contains(&capture_source(envelope.pathway)) {
        return FilterDecision::Ignored {
            filter: CAPTURE_SOURCES.to_owned(),
        };
    }
    if envelope.pathway == Pathway::Entered {
        return FilterDecision::Gather {
            filter: ENTERED_BYPASS.to_owned(),
            kinds: Vec::new(),
        };
    }
    composite(filters, envelope)
}

#[must_use]
pub fn composite(filters: &[Arc<dyn EventFilter>], envelope: &BrainEnvelope) -> FilterDecision {
    if filters.is_empty() {
        return FilterDecision::NoFilter;
    }
    for filter in filters {
        match filter.decide(envelope) {
            FilterVerdict::Gather { kinds } => {
                return FilterDecision::Gather {
                    filter: filter.name().to_owned(),
                    kinds,
                };
            },
            FilterVerdict::Ignore => {
                return FilterDecision::Ignored {
                    filter: filter.name().to_owned(),
                };
            },
            FilterVerdict::Abstain => {},
        }
    }
    FilterDecision::Ignored {
        filter: ALL_ABSTAINED.to_owned(),
    }
}

const fn capture_source(pathway: Pathway) -> CaptureSource {
    match pathway {
        Pathway::DocumentRevision => CaptureSource::Document,
        Pathway::CommentResolved => CaptureSource::Comment,
        Pathway::Entered => CaptureSource::Entered,
    }
}
