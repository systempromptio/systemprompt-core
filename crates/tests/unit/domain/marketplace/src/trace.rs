//! Manifest drop tracing.
//!
//! A trace answers "why is skill X missing from the manifest". Every stage and
//! kind carries two independent spellings — a hand-written `Display` label and
//! a serde `kebab-case` rename — and a diagnostic reading the serialised trace
//! must name the same stage as one reading the rendered output. Nothing forces
//! them to agree, so they are asserted against each other rather than each
//! being pinned to a literal.

use systemprompt_marketplace::{
    ManifestTrace, NoopTrace, TraceEvent, TraceKind, TraceSink, TraceStage,
};

const STAGES: [TraceStage; 7] = [
    TraceStage::DiskScan,
    TraceStage::Parse,
    TraceStage::Disabled,
    TraceStage::MarketplaceScope,
    TraceStage::PluginSelection,
    TraceStage::AccessFilter,
    TraceStage::OrphanPrune,
];

const KINDS: [TraceKind; 5] = [
    TraceKind::Skill,
    TraceKind::Agent,
    TraceKind::McpServer,
    TraceKind::Artifact,
    TraceKind::Plugin,
];

macro_rules! wire {
    ($value:expr) => {
        serde_json::to_value(&$value)
            .expect("serialise")
            .as_str()
            .expect("stages and kinds serialise as strings")
            .to_owned()
    };
}

// Why: the two spellings are written independently — one in `Display`, one by
// serde's rename. A tool reading the JSON trace and a human reading the
// rendered line must be talking about the same stage.
#[test]
fn every_stage_displays_exactly_as_it_serialises() {
    for stage in STAGES {
        assert_eq!(
            stage.to_string(),
            wire!(stage),
            "{stage:?} renders and serialises differently"
        );
    }
}

#[test]
fn every_kind_displays_exactly_as_it_serialises() {
    for kind in KINDS {
        assert_eq!(
            kind.to_string(),
            wire!(kind),
            "{kind:?} renders and serialises differently"
        );
    }
}

// Why: the labels are the vocabulary a caller greps for. Multi-word variants
// are the ones a rename would silently change, so the hyphenation is pinned.
#[test]
fn multi_word_labels_are_hyphenated_rather_than_run_together() {
    assert_eq!(TraceStage::DiskScan.to_string(), "disk-scan");
    assert_eq!(
        TraceStage::MarketplaceScope.to_string(),
        "marketplace-scope"
    );
    assert_eq!(TraceStage::PluginSelection.to_string(), "plugin-selection");
    assert_eq!(TraceStage::AccessFilter.to_string(), "access-filter");
    assert_eq!(TraceStage::OrphanPrune.to_string(), "orphan-prune");
    assert_eq!(TraceKind::McpServer.to_string(), "mcp-server");
}

// Why: no two stages may share a label. A collision makes two different drop
// reasons indistinguishable in the trace, which is the one thing the trace
// exists to tell apart.
#[test]
fn no_two_stages_or_kinds_share_a_label() {
    let mut stage_labels: Vec<String> = STAGES.iter().map(ToString::to_string).collect();
    stage_labels.sort();
    let before = stage_labels.len();
    stage_labels.dedup();
    assert_eq!(before, stage_labels.len(), "two stages share a label");

    let mut kind_labels: Vec<String> = KINDS.iter().map(ToString::to_string).collect();
    kind_labels.sort();
    let before = kind_labels.len();
    kind_labels.dedup();
    assert_eq!(before, kind_labels.len(), "two kinds share a label");
}

// Why: the production path uses `NoopTrace` and must cost nothing. If it
// retained events, every manifest assembly would accumulate them for a caller
// that never reads them.
#[test]
fn the_noop_sink_discards_what_it_is_given() {
    let mut sink = NoopTrace;

    for stage in STAGES {
        sink.record(TraceEvent {
            kind: TraceKind::Skill,
            id: "s1".to_owned(),
            stage,
            reason: "dropped".to_owned(),
        });
    }
}

// Why: the recording sink keeps events in the order they happened. A trace
// reordered is a trace that misattributes which stage dropped an entry first.
#[test]
fn the_recording_sink_keeps_every_event_in_order() {
    let mut sink = ManifestTrace::default();
    assert!(sink.events.is_empty(), "a fresh trace has recorded nothing");

    for (index, stage) in STAGES.into_iter().enumerate() {
        sink.record(TraceEvent {
            kind: TraceKind::Skill,
            id: format!("skill-{index}"),
            stage,
            reason: format!("reason-{index}"),
        });
    }

    assert_eq!(sink.events.len(), STAGES.len());
    for (index, event) in sink.events.iter().enumerate() {
        assert_eq!(
            event.id,
            format!("skill-{index}"),
            "events are out of order"
        );
        assert_eq!(event.stage, STAGES[index]);
    }
}

// Why: the serialised event is what a diagnostic caller parses. Its field
// names are the contract, and `kind` and `stage` must arrive as their labels
// rather than as Rust variant names.
#[test]
fn a_recorded_event_serialises_with_its_labels() {
    let event = TraceEvent {
        kind: TraceKind::McpServer,
        id: "weather".to_owned(),
        stage: TraceStage::AccessFilter,
        reason: "not visible to this user".to_owned(),
    };

    let json = serde_json::to_value(&event).expect("serialise event");

    assert_eq!(json["kind"], "mcp-server");
    assert_eq!(json["stage"], "access-filter");
    assert_eq!(json["id"], "weather");
    assert_eq!(json["reason"], "not visible to this user");
}
