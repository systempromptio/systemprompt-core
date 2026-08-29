//! `JudgeService::score` — the LLM-judge scoring path.
//!
//! The judge's whole contract is that it refuses to believe the model. It
//! constrains the response to a schema, parses it, and rejects a score outside
//! the rubric's scale before anything downstream records a verdict. These
//! drive the queued mock provider so each of those refusals is exercised with
//! a real response body rather than a stub error.
//!
//! `request_cost` returns 0 for an unknown request id, so the cost lookup
//! needs no seeded row unless the assertion is about cost itself.

use std::sync::Arc;

use systemprompt_evaluation::models::{Rubric, RubricDimension, Verdict};
use systemprompt_evaluation::repository::SamplingRepository;
use systemprompt_evaluation::services::{JudgeService, JudgeSpec, JudgeTarget};
use systemprompt_identifiers::{ContextId, EvalRubricId, UserId};
use systemprompt_models::ai::{AiResponse, DynAiProvider};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use systemprompt_test_mocks::MockAiProvider;

fn rubric(pass_threshold: i32) -> Rubric {
    Rubric {
        id: EvalRubricId::new("rubric-judge-test"),
        name: "judge-test".to_owned(),
        dimensions: vec![RubricDimension {
            name: "accuracy".to_owned(),
            description: "Is the answer correct".to_owned(),
            weight: 1.0,
        }],
        pass_threshold,
        prompt_template: None,
        enabled: true,
    }
}

fn target() -> JudgeTarget {
    JudgeTarget {
        transcript: "user: what is 2+2".to_owned(),
        response: "4".to_owned(),
        expectation: Some("should answer 4".to_owned()),
    }
}

fn verdict_json(score: i32, repair_hint: Option<&str>) -> String {
    let hint = repair_hint.map_or("null".to_owned(), |h| format!("\"{h}\""));
    format!(
        r#"{{"overall_score":{score},"dimension_scores":[],"rationale":"because","repair_hint":{hint}}}"#
    )
}

fn ai_response(content: &str) -> AiResponse {
    let mut resp = AiResponse::default();
    resp.request_id = uuid::Uuid::new_v4();
    resp.content = content.to_owned();
    resp.provider = "mock".to_owned();
    resp.model = "mock-judge".to_owned();
    resp
}

async fn judge_returning(content: &str) -> JudgeService {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await.expect("pool");
    let sampling = SamplingRepository::new(&pool).expect("sampling repo");
    let ai: DynAiProvider = Arc::new(
        MockAiProvider::builder()
            .with_generate_response(Ok(ai_response(content)))
            .build(),
    );
    JudgeService::new(
        ai,
        sampling,
        JudgeSpec {
            provider: "mock".to_owned(),
            model: "mock-judge".to_owned(),
            created_by: UserId::new("judge-test-user"),
            run_context: ContextId::generate(),
        },
    )
}

#[tokio::test]
async fn a_score_at_or_above_the_threshold_passes() {
    let judge = judge_returning(&verdict_json(4, None)).await;

    let scored = judge.score(&rubric(4), &target()).await.expect("score");

    assert_eq!(scored.outcome, Verdict::Pass);
    assert_eq!(scored.verdict.overall_score, 4);
}

// Why: Partial is exactly one below the threshold, not "anything between".
// Collapsing it into Fail would lose the signal the repair loop keys on.
#[tokio::test]
async fn one_below_the_threshold_is_partial_and_two_below_is_fail() {
    let judge = judge_returning(&verdict_json(3, None)).await;
    let scored = judge.score(&rubric(4), &target()).await.expect("score");
    assert_eq!(scored.outcome, Verdict::Partial, "one below threshold");

    let judge = judge_returning(&verdict_json(2, None)).await;
    let scored = judge.score(&rubric(4), &target()).await.expect("score");
    assert_eq!(scored.outcome, Verdict::Fail, "two below threshold");
}

#[tokio::test]
async fn a_repair_hint_survives_into_the_verdict() {
    let judge = judge_returning(&verdict_json(2, Some("cite the source"))).await;

    let scored = judge.score(&rubric(4), &target()).await.expect("score");

    assert_eq!(
        scored.verdict.repair_hint.as_deref(),
        Some("cite the source")
    );
}

// Why: the model is constrained to a schema but not bound by it. A response
// that is not the agreed shape must be refused rather than recorded as a
// verdict nobody can interpret.
#[tokio::test]
async fn a_response_that_is_not_the_agreed_shape_is_refused() {
    let judge = judge_returning("I think it was pretty good, honestly").await;

    let err = judge
        .score(&rubric(4), &target())
        .await
        .expect_err("unparseable judge output must not score");

    assert!(
        format!("{err}").to_lowercase().contains("judge"),
        "the error should name the judge parse failure: {err}"
    );
}

// Why: a score outside the rubric's scale is the model ignoring the scale, and
// it would otherwise map through `outcome()` into a real Pass or Fail.
#[tokio::test]
async fn a_score_outside_the_scale_is_refused_at_both_ends() {
    for score in [0, 6, -3, 99] {
        let judge = judge_returning(&verdict_json(score, None)).await;

        let err = judge
            .score(&rubric(4), &target())
            .await
            .expect_err("score {score} is outside 1-5 and must be refused");

        assert!(
            format!("{err}").contains(&score.to_string()),
            "the error should name the offending score {score}: {err}"
        );
    }
}

#[tokio::test]
async fn an_unknown_request_id_costs_nothing_rather_than_failing() {
    let judge = judge_returning(&verdict_json(5, None)).await;

    let scored = judge.score(&rubric(4), &target()).await.expect("score");

    assert_eq!(
        scored.judge_cost_microdollars, 0,
        "a judge request with no ai_requests row costs 0, not an error"
    );
}

#[tokio::test]
async fn a_provider_failure_surfaces_as_an_ai_error() {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await.expect("pool");
    let sampling = SamplingRepository::new(&pool).expect("sampling repo");
    let ai: DynAiProvider = Arc::new(
        MockAiProvider::builder()
            .with_generate_error(anyhow::anyhow!("upstream refused"))
            .build(),
    );
    let judge = JudgeService::new(
        ai,
        sampling,
        JudgeSpec {
            provider: "mock".to_owned(),
            model: "mock-judge".to_owned(),
            created_by: UserId::new("judge-test-user"),
            run_context: ContextId::generate(),
        },
    );

    let err = judge
        .score(&rubric(4), &target())
        .await
        .expect_err("a provider failure must not produce a verdict");

    assert!(
        format!("{err}").contains("upstream refused"),
        "the provider's own message should survive: {err}"
    );
}
