use chrono::Utc;
use systemprompt_brain::analyses::bloat::{bloat_decision, bloat_drafts};
use systemprompt_brain::analyses::coverage::{coverage_drafts, under_covered};
use systemprompt_brain::analyses::relevance::{relevance_drafts, weight_adjustment};
use systemprompt_brain::analyses::{
    BLOAT_ANALYSIS, BloatAction, BloatThresholds, CoverageThresholds, KindUsage, ProjectRecalls,
    RelevanceThresholds, WorkflowKindUsage, fresh_drafts,
};
use systemprompt_identifiers::{BrainProposalId, WorkflowId};
use systemprompt_models::brain::{BrainProposal, ProposalDraft, ProposalKind, ProposalStatus};

fn usage(kind: &str, facts: i64, injections: i64, citations: i64) -> KindUsage {
    KindUsage {
        kind: crate::fixtures::kind(kind),
        facts,
        injections,
        citations,
    }
}

fn cited(workflow: Option<&str>, kind: &str, injections: i64, citations: i64) -> WorkflowKindUsage {
    WorkflowKindUsage {
        workflow: workflow.map(|w| WorkflowId::try_new(w).expect("workflow")),
        kind: crate::fixtures::kind(kind),
        injections,
        citations,
    }
}

fn draft(analysis: &str, body: serde_json::Value) -> ProposalDraft {
    ProposalDraft {
        analysis: analysis.to_owned(),
        kind: ProposalKind::ConfigDiff,
        summary: "s".to_owned(),
        body,
    }
}

fn open(draft: &ProposalDraft) -> BrainProposal {
    BrainProposal {
        id: BrainProposalId::generate(),
        analysis: draft.analysis.clone(),
        kind: draft.kind,
        status: ProposalStatus::Open,
        summary: "other summary".to_owned(),
        body: draft.body.clone(),
        created_at: Utc::now(),
        decided_by: None,
        decided_at: None,
    }
}

#[test]
fn few_facts_are_never_bloat() {
    let t = BloatThresholds::default();
    assert_eq!(bloat_decision(&usage("a", t.min_facts - 1, 0, 0), t), None);
}

#[test]
fn many_facts_never_injected_retire_the_kind() {
    let t = BloatThresholds::default();
    assert_eq!(
        bloat_decision(&usage("a", 500, 0, 0), t),
        Some(BloatAction::RetireKind)
    );
}

#[test]
fn rare_injection_raises_materiality_and_common_injection_does_not() {
    let t = BloatThresholds::default();
    assert_eq!(
        bloat_decision(&usage("a", 1000, 10, 0), t),
        Some(BloatAction::RaiseMateriality)
    );
    assert_eq!(bloat_decision(&usage("a", 1000, 11, 0), t), None);
}

#[test]
fn bloat_draft_is_a_config_diff_naming_the_kind() {
    let drafts = bloat_drafts(
        &[usage("noisy", 500, 0, 0), usage("ok", 500, 400, 0)],
        BloatThresholds::default(),
    )
    .expect("drafts");
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].analysis, BLOAT_ANALYSIS);
    assert_eq!(drafts[0].kind, ProposalKind::ConfigDiff);
    assert_eq!(drafts[0].body["kind"], "noisy");
    assert_eq!(drafts[0].body["action"], "retire_kind");
}

#[test]
fn relevance_lowers_uncited_and_raises_cited_kinds() {
    let t = RelevanceThresholds::default();
    let low = weight_adjustment(&cited(None, "low", 100, 1), t).expect("low");
    assert!(low.weight_factor < 1.0);
    let high = weight_adjustment(&cited(None, "high", 100, 80), t).expect("high");
    assert!(high.weight_factor > 1.0);
    assert!(weight_adjustment(&cited(None, "mid", 100, 20), t).is_none());
    assert!(weight_adjustment(&cited(None, "thin", t.min_injections - 1, 0), t).is_none());
}

#[test]
fn relevance_drafts_one_diff_or_none() {
    let t = RelevanceThresholds::default();
    assert!(
        relevance_drafts(&[cited(None, "mid", 100, 20)], t)
            .expect("none")
            .is_empty()
    );
    let drafts = relevance_drafts(
        &[cited(None, "low", 100, 1), cited(None, "high", 100, 80)],
        t,
    )
    .expect("one");
    assert_eq!(drafts.len(), 1);
    assert_eq!(
        drafts[0].body["adjustments"].as_array().map(Vec::len),
        Some(2)
    );
}

#[test]
fn relevance_adjusts_a_kind_per_workflow() {
    let t = RelevanceThresholds::default();
    let usage = [
        cited(Some("billing"), "decision", 100, 1),
        cited(Some("support"), "decision", 100, 80),
        cited(None, "decision", 100, 20),
    ];
    let adjusted: Vec<_> = usage
        .iter()
        .filter_map(|u| weight_adjustment(u, t))
        .map(|a| {
            (
                a.workflow.map(|w| w.as_str().to_owned()),
                a.weight_factor > 1.0,
            )
        })
        .collect();
    assert_eq!(
        adjusted,
        vec![
            (Some("billing".to_owned()), false),
            (Some("support".to_owned()), true),
        ]
    );
}

#[test]
fn coverage_flags_projects_whose_recalls_come_back_empty() {
    let t = CoverageThresholds::default();
    let empty = ProjectRecalls {
        project: Some("billing".to_owned()),
        recalls: 40,
        empty: 30,
    };
    let fine = ProjectRecalls {
        project: None,
        recalls: 40,
        empty: 5,
    };
    assert!(under_covered(&empty, t));
    assert!(!under_covered(&fine, t));
    let drafts = coverage_drafts(&[empty, fine], t).expect("drafts");
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].kind, ProposalKind::FactActions);
}

#[test]
fn a_draft_matching_an_open_proposal_is_not_stored_again() {
    let first = draft(BLOAT_ANALYSIS, serde_json::json!({"kind": "a", "facts": 1}));
    let reordered = draft(BLOAT_ANALYSIS, serde_json::json!({"facts": 1, "kind": "a"}));
    let other = draft(BLOAT_ANALYSIS, serde_json::json!({"kind": "b", "facts": 1}));
    let fresh = fresh_drafts(vec![reordered, other.clone()], &[open(&first)]);
    assert_eq!(fresh, vec![other]);
}

#[test]
fn the_same_body_from_another_analysis_is_kept_and_repeats_collapse() {
    let body = serde_json::json!({"kind": "a"});
    let a = draft("bloat", body.clone());
    let b = draft("custom", body);
    let fresh = fresh_drafts(vec![a.clone(), a.clone(), b.clone()], &[]);
    assert_eq!(fresh, vec![a, b]);
}
