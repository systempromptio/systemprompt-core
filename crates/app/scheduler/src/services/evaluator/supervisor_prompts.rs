//! Prompt construction and bounded client output parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

pub(super) fn execution_prompt(case: &CaseContent) -> SchedulerResult<String> {
    Ok(format!(
        "Execute this immutable evaluation case. Use only the installed skills and configured evaluation fixture MCP service. Do not use a shell or network. Make any requested platform test-record write at most once, read it back, and restore it. Your final response must answer the case and cite the fixture/tool evidence used.\n\nCASE PROMPT:\n{}\n\nEXPECTED BEHAVIOURS (do not merely repeat these):\n{}\n\nNAMED DETERMINISTIC ASSERTIONS:\n{}",
        case.prompt,
        serde_json::to_string(&case.expected_behavior).map_err(internal)?,
        serde_json::to_string(&case.assertions).map_err(internal)?,
    ))
}

pub(super) fn judgment_prompt(
    case: &CaseContent,
    rubric: &RubricContent,
    evidence: &[&String],
) -> SchedulerResult<String> {
    Ok(format!(
        "Judge the completed evaluation using only the retained files listed below. Read evidence/client-events.jsonl when needed. Return only one JSON object matching {{\"dimensions\":[{{\"name\":string,\"score\":integer 1..5,\"evidence\":[exact retained reference]}}],\"hard_gates\":{{string:boolean}},\"rationale\":string}}. Include every rubric dimension and exactly every hard gate. Never invent a reference. A missing or ambiguous fact must reduce the score.\n\nCASE:\n{}\n\nEXPECTED:\n{}\n\nRUBRIC:\n{}\n\nRETAINED REFERENCES:\n{}",
        case.prompt,
        serde_json::to_string(&case.expected_behavior).map_err(internal)?,
        serde_json::to_string(rubric).map_err(internal)?,
        serde_json::to_string(evidence).map_err(internal)?,
    ))
}

pub(super) fn parse_judgment(bytes: &[u8]) -> SchedulerResult<EvidenceJudgment> {
    let body = std::str::from_utf8(bytes).map_err(internal)?;
    for line in body.lines().rev() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let candidate = event
            .get("result")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                event
                    .pointer("/message/content/0/text")
                    .and_then(serde_json::Value::as_str)
            });
        let Some(candidate) = candidate else {
            continue;
        };
        if let Ok(judgment) = serde_json::from_str(candidate) {
            return Ok(judgment);
        }
        if let (Some(start), Some(end)) = (candidate.find('{'), candidate.rfind('}')) {
            if start <= end {
                if let Ok(judgment) = serde_json::from_str(&candidate[start..=end]) {
                    return Ok(judgment);
                }
            }
        }
    }
    Err(SchedulerError::config_error(
        "Semantic judge returned no valid evidence judgment",
    ))
}

pub(super) fn parse_suggestion(bytes: &[u8]) -> SchedulerResult<GeneratedSuggestion> {
    parse_client_json(bytes, "suggestion")
}

fn parse_client_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    label: &str,
) -> SchedulerResult<T> {
    let body = std::str::from_utf8(bytes).map_err(internal)?;
    for line in body.lines().rev() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let candidate = event
            .get("result")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                event
                    .pointer("/message/content/0/text")
                    .and_then(serde_json::Value::as_str)
            });
        let Some(candidate) = candidate else {
            continue;
        };
        if let Ok(value) = serde_json::from_str(candidate) {
            return Ok(value);
        }
        if let (Some(start), Some(end)) = (candidate.find('{'), candidate.rfind('}')) {
            if start <= end {
                if let Ok(value) = serde_json::from_str(&candidate[start..=end]) {
                    return Ok(value);
                }
            }
        }
    }
    Err(SchedulerError::config_error(format!(
        "Client returned no valid {label} JSON"
    )))
}

pub(super) fn suggestion_prompt(
    case: &CaseContent,
    failures: &[String],
    evidence: &[&String],
) -> SchedulerResult<String> {
    Ok(format!(
        "Using only the retained development-case evidence, propose a candidate skill change. Never use or reveal holdout content. Return only one JSON object matching {{\"proposed_changes\":object,\"hypothesis\":string,\"supporting_failures\":[string],\"originating_evidence\":[exact retained reference]}}.\n\nCASE:\n{}\n\nFAILURES:\n{}\n\nEVIDENCE:\n{}",
        case.prompt,
        serde_json::to_string(failures).map_err(internal)?,
        serde_json::to_string(evidence).map_err(internal)?,
    ))
}
