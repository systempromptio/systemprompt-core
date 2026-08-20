//! Orchestration service for evaluation runs (sample, judge, replay).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{ContextId, EvalCaseId, EvalRunId, UserId};
use systemprompt_models::ai::DynAiProvider;

use crate::error::Result;
use crate::models::{
    EvalResult, EvalRun, EvalRunKind, NewCaseParams, NewRunParams, Rubric, RubricDimension,
    SampleFilter, TriggerSource,
};
use crate::repository::{
    EvalCaseRepository, EvalRepositories, EvalResultRepository, EvalRubricRepository,
    EvalRunRepository, SamplingRepository,
};
use crate::services::judge::{JudgeService, JudgeSpec};
use crate::services::loop_runner::{AutoImproveLoop, LoopLimits, LoopReport};
use crate::services::replay::ReplayService;
use crate::services::sampler::SamplerService;

const DEFAULT_RUBRIC: &str = "default";

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub judge_provider: String,
    pub judge_model: String,
    pub rubric_name: Option<String>,
    pub filter: SampleFilter,
    pub budget_microdollars: Option<i64>,
    pub created_by: UserId,
    pub trigger_source: TriggerSource,
}

#[derive(Clone)]
pub struct EvaluationService {
    ai: DynAiProvider,
    runs: EvalRunRepository,
    cases: EvalCaseRepository,
    results: EvalResultRepository,
    rubrics: EvalRubricRepository,
    sampling: SamplingRepository,
}

impl std::fmt::Debug for EvaluationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluationService")
            .field("runs", &self.runs)
            .finish_non_exhaustive()
    }
}

impl EvaluationService {
    pub fn new(repositories: EvalRepositories, ai: DynAiProvider) -> Self {
        let EvalRepositories {
            runs,
            cases,
            results,
            rubrics,
            sampling,
        } = repositories;
        Self {
            ai,
            runs,
            cases,
            results,
            rubrics,
            sampling,
        }
    }

    pub async fn run_judge(&self, request: RunRequest) -> Result<(EvalRunId, LoopReport)> {
        self.run_with_kind(EvalRunKind::Judge, request).await
    }

    pub async fn replay_failures(
        &self,
        source_run: &EvalRunId,
        mut request: RunRequest,
    ) -> Result<(EvalRunId, LoopReport)> {
        let failures = self.results.failures_for_replay(source_run).await?;
        let ids: Vec<String> = failures
            .iter()
            .filter_map(|r| r.ai_request_id.as_ref())
            .map(|id| id.as_str().to_owned())
            .collect();
        if ids.is_empty() {
            return Err(crate::error::EvaluationError::ReplaySource(format!(
                "run {source_run} has no unrepaired failures with a source request"
            )));
        }
        request.filter.limit = i64::try_from(ids.len()).unwrap_or(i64::MAX);
        request.filter = request.filter.ids(ids);
        request.filter.since = None;
        self.run_with_kind(EvalRunKind::Replay, request).await
    }

    pub async fn list_runs(&self, limit: i64) -> Result<Vec<EvalRun>> {
        self.runs.list_recent(limit).await
    }

    async fn run_with_kind(
        &self,
        kind: EvalRunKind,
        request: RunRequest,
    ) -> Result<(EvalRunId, LoopReport)> {
        let rubric = self.resolve_rubric(request.rubric_name.as_deref()).await?;
        let run_id = self
            .runs
            .create(&NewRunParams {
                kind,
                judge_provider: request.judge_provider.clone(),
                judge_model: request.judge_model.clone(),
                sample_size: i32::try_from(request.filter.limit).unwrap_or(i32::MAX),
                created_by: request.created_by.clone(),
                rubric_id: Some(rubric.id.clone()),
                trigger_source: request.trigger_source,
            })
            .await?;

        let run_context = ContextId::derived_from_evaluation_run(&run_id);
        let judge = JudgeService::new(
            Arc::clone(&self.ai),
            self.sampling.clone(),
            JudgeSpec {
                provider: request.judge_provider,
                model: request.judge_model,
                created_by: request.created_by.clone(),
                run_context: run_context.clone(),
            },
        );
        let auto_improve = AutoImproveLoop {
            sampler: SamplerService::new(self.sampling.clone()),
            judge,
            replay: ReplayService::new(Arc::clone(&self.ai), request.created_by, run_context),
            runs: self.runs.clone(),
            results: self.results.clone(),
        };

        let outcome = auto_improve
            .run(
                &run_id,
                &rubric,
                &request.filter,
                LoopLimits {
                    budget_microdollars: request.budget_microdollars,
                },
            )
            .await;
        match outcome {
            Ok(report) => {
                self.runs.complete(&run_id).await?;
                Ok((run_id, report))
            },
            Err(e) => {
                self.runs.fail(&run_id, &e.to_string()).await?;
                Err(e)
            },
        }
    }

    pub async fn promote_case(&self, params: &NewCaseParams) -> Result<EvalCaseId> {
        self.cases.create(params).await
    }

    pub async fn get_run(&self, run_id: &EvalRunId) -> Result<EvalRun> {
        self.runs.get(run_id).await
    }

    pub async fn list_results(&self, run_id: &EvalRunId) -> Result<Vec<EvalResult>> {
        self.results.list_by_run(run_id).await
    }

    pub async fn sample(
        &self,
        filter: &SampleFilter,
    ) -> Result<Vec<crate::models::SampledRequest>> {
        SamplerService::new(self.sampling.clone())
            .sample(filter)
            .await
    }

    async fn resolve_rubric(&self, name: Option<&str>) -> Result<Rubric> {
        let name = name.unwrap_or(DEFAULT_RUBRIC);
        match self.rubrics.get_by_name(name).await {
            Ok(rubric) => Ok(rubric),
            Err(crate::error::EvaluationError::RubricNotFound(_)) if name == DEFAULT_RUBRIC => {
                let rubric = default_rubric();
                self.rubrics.upsert(&rubric).await?;
                Ok(rubric)
            },
            Err(e) => Err(e),
        }
    }
}

fn default_rubric() -> Rubric {
    Rubric {
        id: systemprompt_identifiers::EvalRubricId::generate(),
        name: DEFAULT_RUBRIC.to_owned(),
        dimensions: vec![
            RubricDimension {
                name: "correctness".to_owned(),
                description: "The response is factually and technically accurate.".to_owned(),
                weight: 1.0,
            },
            RubricDimension {
                name: "helpfulness".to_owned(),
                description: "The response addresses what the user actually asked.".to_owned(),
                weight: 1.0,
            },
            RubricDimension {
                name: "completeness".to_owned(),
                description: "The response covers the request without gaps.".to_owned(),
                weight: 1.0,
            },
        ],
        pass_threshold: 4,
        prompt_template: None,
        enabled: true,
    }
}
