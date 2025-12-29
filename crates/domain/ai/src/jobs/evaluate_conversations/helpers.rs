use anyhow::{anyhow, Result};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::a2a::{Message, Part};
use systemprompt_models::ai::request::{AiMessage, AiRequest};
use systemprompt_models::ai::response_format::StructuredOutputOptions;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::{AiEvaluationResponse, ConversationEvaluation};

use super::prompt::EVALUATION_PROMPT;
use crate::repository::EvaluationRepository;
use crate::AiService;

struct ConversationMetadata {
    agent_name: String,
    duration_seconds: i32,
}

fn extract_conversation_metadata(conversation: &serde_json::Value) -> ConversationMetadata {
    let agent_name = conversation
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let started_at = conversation
        .get("started_at")
        .and_then(systemprompt_core_database::parse_database_datetime);
    let completed_at = conversation
        .get("completed_at")
        .and_then(systemprompt_core_database::parse_database_datetime);

    let duration_seconds = match (started_at, completed_at) {
        (Some(start), Some(end)) => (end - start).num_seconds().max(0) as i32,
        _ => 0,
    };

    ConversationMetadata {
        agent_name,
        duration_seconds,
    }
}

fn parse_evaluation_response(json: &str) -> Result<AiEvaluationResponse> {
    serde_json::from_str(json).map_err(|e| {
        let preview_len = std::cmp::min(500, json.len());
        anyhow!(
            "Failed to parse AI evaluation response: {} | JSON: {}",
            e,
            &json[..preview_len]
        )
    })
}

fn log_evaluation_result(context_id: &str, eval: &ConversationEvaluation) {
    if eval.user_satisfied == 0 && eval.conversation_quality == 0 {
        tracing::warn!(
            context_id = %context_id,
            quality = eval.conversation_quality,
            user_satisfied = eval.user_satisfied,
            score = eval.overall_score,
            "Zero-score conversation detected"
        );
    }

    tracing::debug!(
        context_id = %context_id,
        quality = eval.conversation_quality,
        user_satisfied = eval.user_satisfied,
        score = eval.overall_score,
        goal_achieved = %eval.goal_achieved,
        "Evaluated conversation"
    );
}

pub async fn evaluate_single_conversation(
    context_id: &str,
    conversation: &serde_json::Value,
    ai_service: &AiService,
    repository: &EvaluationRepository,
) -> Result<()> {
    let metadata = extract_conversation_metadata(conversation);
    let messages = get_context_messages_with_content(conversation, repository).await?;

    if messages.is_empty() {
        return Err(anyhow!("No messages found for context"));
    }

    let conversation_text = reconstruct_conversation(&messages)?;
    let total_turns = messages.len() as i32;

    let req_context = create_evaluation_request_context(context_id);
    let evaluation_json = call_ai_evaluator(ai_service, &conversation_text, &req_context).await?;

    tracing::debug!(
        json_preview = &evaluation_json[..std::cmp::min(500, evaluation_json.len())],
        "AI response JSON"
    );

    let ai_response = parse_evaluation_response(&evaluation_json)?;
    let eval = ConversationEvaluation::from_ai_response(
        ai_response,
        ContextId::new(context_id),
        metadata.agent_name,
        total_turns,
        metadata.duration_seconds,
    );

    repository.create_evaluation(&eval).await?;
    log_evaluation_result(context_id, &eval);

    Ok(())
}

fn create_evaluation_request_context(_task_id: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new("evaluation-job".to_string()),
        TraceId::new(format!("eval-{}", uuid::Uuid::new_v4())),
        ContextId::new(String::new()),
        AgentName::system(),
    )
    .with_user_id(UserId::new("system".to_string()))
}

async fn get_context_messages_with_content(
    conversation: &serde_json::Value,
    repository: &EvaluationRepository,
) -> Result<Vec<Message>> {
    let context_id_str = conversation
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing context_id"))?;

    let context_id = ContextId::new(context_id_str.to_string());

    repository
        .get_messages_by_context(&context_id)
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to retrieve messages for context {}: {}",
                context_id_str,
                e
            )
        })
}

fn reconstruct_conversation(messages: &[Message]) -> Result<String> {
    let mut conversation_text = String::new();

    for msg in messages {
        let role_label = match msg.role.as_str() {
            "user" => "User",
            "agent" => "Agent",
            _ => "Unknown",
        };

        conversation_text.push_str(&format!("\n{}: ", role_label));

        let mut has_content = false;
        for part in &msg.parts {
            match part {
                Part::Text(text_part) => {
                    conversation_text.push_str(&text_part.text);
                    has_content = true;
                },
                Part::File(file_part) => {
                    let file_name = file_part
                        .file
                        .name
                        .as_deref()
                        .ok_or_else(|| anyhow!("File part missing name"))?;
                    conversation_text.push_str(&format!("[File: {}] ", file_name));
                    has_content = true;
                },
                Part::Data(_) => {
                    conversation_text.push_str("[Data attached] ");
                    has_content = true;
                },
            }
        }

        if !has_content {
            conversation_text.push_str("[Empty message]");
        }
    }

    if conversation_text.is_empty() {
        return Err(anyhow!("Empty conversation - no messages to evaluate"));
    }

    Ok(conversation_text)
}

async fn call_ai_evaluator(
    ai_service: &AiService,
    conversation_text: &str,
    req_context: &RequestContext,
) -> Result<String> {
    let request = AiRequest::builder(
        vec![
            AiMessage::system(EVALUATION_PROMPT),
            AiMessage::user(format!(
                "Evaluate this conversation:\n\n{}",
                conversation_text
            )),
        ],
        ai_service.default_provider(),
        ai_service.default_model(),
        ai_service.default_max_output_tokens(),
        req_context.clone(),
    )
    .with_structured_output(StructuredOutputOptions::with_json_object())
    .build();

    tracing::debug!("Calling AI service for evaluation");

    let response = ai_service.generate(&request).await?;

    tracing::debug!(
        tokens = response.tokens_used.unwrap_or(0),
        "AI evaluation completed"
    );

    Ok(response.content)
}
