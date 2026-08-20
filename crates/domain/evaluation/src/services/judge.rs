//! LLM-judge scoring of sampled request/response pairs against a rubric.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    Actor, AgentName, AiRequestId, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::ai::{
    AiMessage, AiRequest, DynAiProvider, ResponseFormat, StructuredOutputOptions,
};

use crate::error::{EvaluationError, Result};
use crate::models::{JudgeVerdict, Rubric, Verdict};
use crate::repository::SamplingRepository;

const JUDGE_ACTOR_JOB: &str = "evaluation_judge";
const JUDGE_AGENT: &str = "evaluation-judge";
const JUDGE_MAX_OUTPUT_TOKENS: u32 = 2048;
const MAX_JUDGE_CHARS: usize = 8_000;

/// What the judge grades: the prompt transcript and the response under test.
#[derive(Debug, Clone)]
pub struct JudgeTarget {
    pub transcript: String,
    pub response: String,
    pub expectation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredVerdict {
    pub verdict: JudgeVerdict,
    pub outcome: Verdict,
    pub judge_ai_request_id: AiRequestId,
    pub judge_cost_microdollars: i64,
}

#[derive(Debug, Clone)]
pub struct JudgeSpec {
    pub provider: String,
    pub model: String,
    pub created_by: UserId,
    pub run_context: ContextId,
}

#[derive(Clone)]
pub struct JudgeService {
    ai: DynAiProvider,
    sampling: SamplingRepository,
    judge_provider: String,
    judge_model: String,
    created_by: UserId,
    run_context: ContextId,
}

impl std::fmt::Debug for JudgeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JudgeService")
            .field("judge_provider", &self.judge_provider)
            .field("judge_model", &self.judge_model)
            .finish_non_exhaustive()
    }
}

impl JudgeService {
    pub fn new(ai: DynAiProvider, sampling: SamplingRepository, spec: JudgeSpec) -> Self {
        Self {
            ai,
            sampling,
            judge_provider: spec.provider,
            judge_model: spec.model,
            created_by: spec.created_by,
            run_context: spec.run_context,
        }
    }

    pub async fn score(&self, rubric: &Rubric, target: &JudgeTarget) -> Result<ScoredVerdict> {
        let request = self.build_request(rubric, target);
        let response = self
            .ai
            .generate(&request)
            .await
            .map_err(|e| EvaluationError::Ai(e.to_string()))?;

        let verdict: JudgeVerdict = serde_json::from_str(response.content.trim())
            .map_err(|e| EvaluationError::JudgeParse(e.to_string()))?;
        if !(1..=5).contains(&verdict.overall_score) {
            return Err(EvaluationError::JudgeParse(format!(
                "overall_score {} outside 1-5",
                verdict.overall_score
            )));
        }

        let request_id = response.request_id.to_string();
        let judge_cost_microdollars = self.sampling.request_cost(&request_id).await?;
        Ok(ScoredVerdict {
            outcome: outcome(verdict.overall_score, rubric.pass_threshold),
            verdict,
            judge_ai_request_id: AiRequestId::new(request_id),
            judge_cost_microdollars,
        })
    }

    fn build_request(&self, rubric: &Rubric, target: &JudgeTarget) -> AiRequest {
        let context = RequestContext::new(
            SessionId::generate(),
            TraceId::generate(),
            self.run_context.clone(),
            AgentName::new(JUDGE_AGENT),
        )
        .with_actor(Actor::job(self.created_by.clone(), JUDGE_ACTOR_JOB));

        let messages = vec![AiMessage::user(judge_prompt(rubric, target))];
        AiRequest::builder(
            messages,
            self.judge_provider.clone(),
            self.judge_model.clone(),
            JUDGE_MAX_OUTPUT_TOKENS,
            context,
        )
        .with_system_prompt(system_prompt(rubric))
        .with_structured_output(StructuredOutputOptions {
            response_format: Some(ResponseFormat::json_schema(JudgeVerdict::response_schema())),
            ..StructuredOutputOptions::default()
        })
        .build()
    }
}

const fn outcome(score: i32, pass_threshold: i32) -> Verdict {
    if score >= pass_threshold {
        Verdict::Pass
    } else if score == pass_threshold - 1 {
        Verdict::Partial
    } else {
        Verdict::Fail
    }
}

fn system_prompt(rubric: &Rubric) -> String {
    rubric.prompt_template.clone().unwrap_or_else(|| {
        "You are a strict evaluation judge. Score the assistant response \
         against the rubric dimensions on a 1-5 scale, explain your rationale, \
         and when the response falls short provide a concrete repair_hint the \
         assistant could follow to fix it."
            .to_owned()
    })
}

fn judge_prompt(rubric: &Rubric, target: &JudgeTarget) -> String {
    let dimensions = rubric
        .dimensions
        .iter()
        .map(|d| format!("- {}: {}", d.name, d.description))
        .collect::<Vec<_>>()
        .join("\n");
    let expectation = target
        .expectation
        .as_deref()
        .map(|e| format!("\nExpected behaviour:\n{e}\n"))
        .unwrap_or_default();
    format!(
        "Rubric dimensions:\n{dimensions}\n{expectation}\nConversation:\n{}\n\nResponse under \
         evaluation:\n{}",
        truncate(&target.transcript, MAX_JUDGE_CHARS),
        truncate(&target.response, MAX_JUDGE_CHARS)
    )
}

fn truncate(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((idx, _)) => &text[..idx],
        None => text,
    }
}
