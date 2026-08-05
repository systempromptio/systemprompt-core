//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use systemprompt_identifiers::EvalRunId;

use crate::error::{EvaluationError, Result};
use crate::models::{
    DimensionScore, NewResultParams, Rubric, SampleFilter, SampledRequest, Verdict,
};
use crate::repository::{EvalResultRepository, EvalRunRepository};
use crate::services::judge::{JudgeService, JudgeTarget, ScoredVerdict};
use crate::services::replay::ReplayService;
use crate::services::sampler::SamplerService;

const EXCERPT_CHARS: usize = 500;

#[derive(Debug, Clone, Copy)]
pub struct LoopLimits {
    pub budget_microdollars: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LoopReport {
    pub scored: u32,
    pub failed: u32,
    pub replayed: u32,
    pub repaired: u32,
    pub judge_cost_microdollars: i64,
}

/// Sample → judge → repair-hint → replay → re-judge, one pass.
pub struct AutoImproveLoop {
    pub(super) sampler: SamplerService,
    pub(super) judge: JudgeService,
    pub(super) replay: ReplayService,
    pub(super) runs: EvalRunRepository,
    pub(super) results: EvalResultRepository,
}

impl std::fmt::Debug for AutoImproveLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoImproveLoop")
            .field("judge", &self.judge)
            .finish_non_exhaustive()
    }
}

impl AutoImproveLoop {
    pub async fn run(
        &self,
        run_id: &EvalRunId,
        rubric: &Rubric,
        filter: &SampleFilter,
        limits: LoopLimits,
    ) -> Result<LoopReport> {
        let mut report = LoopReport::default();
        let sampled = self.sampler.sample(filter).await?;

        for request in &sampled {
            check_budget(&report, limits)?;
            match self.judge_one(run_id, rubric, request, &mut report).await {
                Ok(()) => {},
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        ai_request_id = %request.ai_request_id,
                        "judge pass failed for sampled request"
                    );
                },
            }
        }
        Ok(report)
    }

    async fn judge_one(
        &self,
        run_id: &EvalRunId,
        rubric: &Rubric,
        request: &SampledRequest,
        report: &mut LoopReport,
    ) -> Result<()> {
        let target = judge_target(request);
        let scored = self.judge.score(rubric, &target).await?;
        report.scored += 1;
        report.judge_cost_microdollars += scored.judge_cost_microdollars;

        let failed = matches!(scored.outcome, Verdict::Fail | Verdict::Partial);
        if failed {
            report.failed += 1;
        }
        let result_id = self
            .results
            .insert(&result_params(run_id, request, &scored, None))
            .await?;
        self.runs
            .record_scored(run_id, failed, scored.judge_cost_microdollars)
            .await?;

        if !failed {
            return Ok(());
        }
        let Some(repair_hint) = scored.verdict.repair_hint.clone().filter(|h| !h.is_empty()) else {
            return Ok(());
        };

        let prompt = request.canonical_prompt();
        let replayed = self.replay.replay(&prompt, &repair_hint).await?;
        report.replayed += 1;

        let repaired_target = JudgeTarget {
            transcript: target.transcript,
            response: replayed.content.clone(),
            expectation: target.expectation,
        };
        let rescored = self.judge.score(rubric, &repaired_target).await?;
        report.judge_cost_microdollars += rescored.judge_cost_microdollars;

        let mut params = result_params(run_id, request, &rescored, Some(result_id.clone()));
        params.response_excerpt = Some(excerpt(&replayed.content));
        self.results.insert(&params).await?;
        self.runs
            .record_scored(
                run_id,
                matches!(rescored.outcome, Verdict::Fail | Verdict::Partial),
                rescored.judge_cost_microdollars,
            )
            .await?;

        if matches!(rescored.outcome, Verdict::Pass) {
            self.results.mark_repaired(&result_id).await?;
            report.repaired += 1;
        }
        Ok(())
    }
}

const fn check_budget(report: &LoopReport, limits: LoopLimits) -> Result<()> {
    match limits.budget_microdollars {
        Some(budget) if report.judge_cost_microdollars >= budget => {
            Err(EvaluationError::BudgetExhausted {
                spent: report.judge_cost_microdollars,
                budget,
            })
        },
        _ => Ok(()),
    }
}

fn judge_target(request: &SampledRequest) -> JudgeTarget {
    let transcript = request
        .messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    JudgeTarget {
        transcript,
        response: request.response_text.clone().unwrap_or_default(),
        expectation: None,
    }
}

fn result_params(
    run_id: &EvalRunId,
    request: &SampledRequest,
    scored: &ScoredVerdict,
    replay_of: Option<systemprompt_identifiers::EvalResultId>,
) -> NewResultParams {
    NewResultParams {
        run_id: run_id.clone(),
        ai_request_id: Some(request.ai_request_id.clone()),
        case_id: None,
        provider: request.provider.clone(),
        model: request.model.clone(),
        overall_score: Some(scored.verdict.overall_score),
        dimension_scores: dimension_scores_json(&scored.verdict.dimension_scores),
        verdict: scored.outcome,
        rationale: Some(scored.verdict.rationale.clone()),
        prompt_excerpt: request.messages.first().map(|m| excerpt(&m.content)),
        response_excerpt: request.response_text.as_deref().map(excerpt),
        judge_cost_microdollars: scored.judge_cost_microdollars,
        repaired: replay_of.is_some(),
        replay_of_result_id: replay_of,
        judge_ai_request_id: Some(scored.judge_ai_request_id.clone()),
    }
}

fn dimension_scores_json(scores: &[DimensionScore]) -> Value {
    match serde_json::to_value(scores) {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize dimension scores");
            Value::Object(serde_json::Map::new())
        },
    }
}

fn excerpt(text: &str) -> String {
    text.chars().take(EXCERPT_CHARS).collect()
}
