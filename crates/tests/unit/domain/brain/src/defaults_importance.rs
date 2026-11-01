use std::collections::BTreeMap;

use systemprompt_brain::defaults::{FormulaImportance, InContextTerms};
use systemprompt_extension::brain::ImportanceModel;
use systemprompt_identifiers::WorkflowId;
use systemprompt_models::brain::{ImportanceConfig, ModelHint, SourceClass};

use crate::fixtures::kind;
use crate::pipeline_fixtures::valid_candidate;

fn config() -> ImportanceConfig {
    let mut config = ImportanceConfig::default();
    config.base.kind_weight.insert(kind("commitment"), 0.5);
    config.base.source_weight.insert(SourceClass::Document, 0.8);
    config.base.model_hint = Some(ModelHint {
        field: "severity".to_owned(),
        weight: 0.2,
    });
    config.base.confirmed_by_owner = 0.1;
    let workflow = WorkflowId::try_new("sales").expect("workflow");
    config.in_context.workflow_weight =
        BTreeMap::from([(workflow, BTreeMap::from([(kind("commitment"), 0.5)]))]);
    config.in_context.subject_match = 0.2;
    config.in_context.recency.half_life_days = 90;
    config.in_context.usage.cited_rate_weight = 0.4;
    config.in_context.usage.ignored_penalty = 0.2;
    config
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

#[test]
fn base_is_kind_times_source_plus_hint_plus_bonus() {
    let mut candidate = valid_candidate();
    candidate
        .fields
        .insert("severity".to_owned(), serde_json::json!(0.5));
    let config = config();
    assert!(close(
        FormulaImportance::base_with(&candidate, &config, false),
        0.5
    ));
    assert!(close(
        FormulaImportance::base_with(&candidate, &config, true),
        0.6
    ));
    assert_eq!(FormulaImportance.base(&candidate, &config), Some(0.5));
}

#[test]
fn the_hint_is_clamped_and_missing_weights_count_as_one() {
    let mut candidate = valid_candidate();
    candidate
        .fields
        .insert("severity".to_owned(), serde_json::json!(3.0));
    assert!(close(
        FormulaImportance::base_with(&candidate, &config(), false),
        0.6
    ));
    assert!(close(
        FormulaImportance::base_with(&valid_candidate(), &ImportanceConfig::default(), false),
        1.0
    ));
}

#[test]
fn score_applies_workflow_recency_subject_and_usage() {
    let commitment = kind("commitment");
    let workflow = WorkflowId::try_new("sales").expect("workflow");
    let terms = InContextTerms {
        base: 0.5,
        kind: &commitment,
        workflow: Some(&workflow),
        age_days: 90.0,
        subject_matched: true,
        cited_rate: 0.5,
        ignored_rate: 0.25,
    };
    assert!(close(FormulaImportance::score(&terms, &config()), 0.475));
    let unmatched = InContextTerms {
        subject_matched: false,
        workflow: None,
        ..terms
    };
    assert!(close(
        FormulaImportance::score(&unmatched, &config()),
        0.5 * 0.5 + 0.15
    ));
}

#[test]
fn an_unconfirmed_candidate_never_earns_the_owner_bonus() {
    let config = config();
    let candidate = valid_candidate();
    let unconfirmed = FormulaImportance::base_with(&candidate, &config, false);
    let confirmed = FormulaImportance::base_with(&candidate, &config, true);
    assert!(close(confirmed - unconfirmed, 0.1));
    assert!(close(unconfirmed, 0.4));
}

#[test]
fn in_context_scores_a_stored_fact_by_workflow_and_recency() {
    use systemprompt_models::auth::RequestActor;
    use systemprompt_models::brain::RecallCtx;

    let mut fact = crate::recall_fixtures::brain_fact(1, "commitment");
    fact.importance_base = 0.8;
    let workflow = WorkflowId::try_new("sales").expect("workflow");
    let ctx = RecallCtx::new(
        crate::fixtures::user("u1"),
        RequestActor::Human,
        fact.updated_at + chrono::Duration::days(90),
    )
    .with_workflow(workflow);
    let score = FormulaImportance
        .in_context(&fact, &ctx, &config())
        .expect("core formula always scores");
    assert!(close(score, 0.8 * 0.5 * 0.5), "{score}");
}
