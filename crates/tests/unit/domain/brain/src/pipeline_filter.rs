use std::sync::Arc;

use systemprompt_brain::pipeline::filter::{
    ALL_ABSTAINED, CAPTURE_SOURCES, ENTERED_BYPASS, FilterDecision, composite, decide,
};
use systemprompt_extension::brain::{EventFilter, FilterVerdict};
use systemprompt_manifest::profile::{BrainCaptureProfile, CaptureSource};
use systemprompt_models::brain::{BrainEnvelope, BrainEventKind};

use crate::fixtures::kind;
use crate::pipeline_fixtures::envelope;

struct Fixed {
    name: &'static str,
    verdict: FilterVerdict,
}

impl EventFilter for Fixed {
    fn name(&self) -> &'static str {
        self.name
    }

    fn decide(&self, _envelope: &BrainEnvelope) -> FilterVerdict {
        self.verdict.clone()
    }
}

fn filter(name: &'static str, verdict: FilterVerdict) -> Arc<dyn EventFilter> {
    Arc::new(Fixed { name, verdict })
}

fn landed() -> BrainEnvelope {
    envelope(BrainEventKind::REVISION_LANDED)
}

#[test]
fn no_filter_registered_gathers_nothing() {
    let decision = decide(&[], &BrainCaptureProfile::default(), &landed());
    assert_eq!(decision, FilterDecision::NoFilter);
    assert_eq!(decision.filter_name(), None);
}

#[test]
fn first_gather_wins_after_abstentions() {
    let filters = vec![
        filter("a", FilterVerdict::Abstain),
        filter("b", FilterVerdict::Gather { kinds: vec![kind("commitment")] }),
        filter("c", FilterVerdict::Ignore),
    ];
    assert_eq!(
        composite(&filters, &landed()),
        FilterDecision::Gather { filter: "b".to_owned(), kinds: vec![kind("commitment")] }
    );
}

#[test]
fn an_earlier_ignore_beats_a_later_gather() {
    let filters = vec![
        filter("drop", FilterVerdict::Ignore),
        filter("keep", FilterVerdict::Gather { kinds: Vec::new() }),
    ];
    assert_eq!(
        composite(&filters, &landed()),
        FilterDecision::Ignored { filter: "drop".to_owned() }
    );
}

#[test]
fn all_abstaining_is_ignored_with_a_reason() {
    let filters = vec![filter("a", FilterVerdict::Abstain), filter("b", FilterVerdict::Abstain)];
    assert_eq!(
        composite(&filters, &landed()),
        FilterDecision::Ignored { filter: ALL_ABSTAINED.to_owned() }
    );
}

#[test]
fn an_entered_fact_bypasses_filters() {
    let filters = vec![filter("drop", FilterVerdict::Ignore)];
    let decision = decide(
        &filters,
        &BrainCaptureProfile::default(),
        &envelope(BrainEventKind::FACT_ENTERED),
    );
    assert_eq!(
        decision,
        FilterDecision::Gather { filter: ENTERED_BYPASS.to_owned(), kinds: Vec::new() }
    );
}

#[test]
fn an_uncaptured_pathway_is_ignored_even_when_a_filter_gathers() {
    let filters = vec![filter("keep", FilterVerdict::Gather { kinds: Vec::new() })];
    let capture = BrainCaptureProfile {
        sources: vec![CaptureSource::Comment],
        ..BrainCaptureProfile::default()
    };
    assert_eq!(
        decide(&filters, &capture, &landed()),
        FilterDecision::Ignored { filter: CAPTURE_SOURCES.to_owned() }
    );
}
