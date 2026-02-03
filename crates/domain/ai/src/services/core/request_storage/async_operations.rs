use crate::models::AiRequestRecord;
use crate::repository::{AiRequestRepository, InsertToolCallParams};
use systemprompt_identifiers::{AiRequestId, SessionId, SessionSource, UserId};
use systemprompt_traits::{AiSessionProvider, CreateAiSessionParams};
use tracing::error;

use super::record_builder::{MessageData, ToolCallData};

pub async fn store_request_async(
    repo: &AiRequestRepository,
    record: &AiRequestRecord,
) -> Option<AiRequestId> {
    repo.insert(record)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store AI request record");
            e
        })
        .ok()
}

pub async fn store_messages_async(
    repo: &AiRequestRepository,
    db_id: &AiRequestId,
    messages: Vec<MessageData>,
) {
    for message in messages {
        if let Err(e) = repo
            .insert_message(db_id, &message.role, &message.content, message.sequence)
            .await
        {
            error!(
                error = %e,
                request_id = %db_id,
                sequence = message.sequence,
                "Failed to store AI request message"
            );
        }
    }
}

pub async fn store_tool_calls_async(
    repo: &AiRequestRepository,
    db_id: &AiRequestId,
    tool_calls: Vec<ToolCallData>,
) {
    for tool_call in tool_calls {
        if let Err(e) = repo
            .insert_tool_call(InsertToolCallParams {
                request_id: db_id,
                ai_tool_call_id: &tool_call.ai_tool_call_id,
                tool_name: &tool_call.tool_name,
                tool_input: &tool_call.tool_input,
                sequence_number: tool_call.sequence,
            })
            .await
        {
            error!(
                error = %e,
                request_id = %db_id,
                tool_name = %tool_call.tool_name,
                "Failed to store AI tool call"
            );
        }
    }
}

pub async fn update_session_usage_async(
    session_provider: &dyn AiSessionProvider,
    user_id: &UserId,
    session_id: Option<&SessionId>,
    tokens: Option<i32>,
    cost_microdollars: i64,
) {
    if user_id.as_str() == "system" {
        return;
    }

    let Some(session_id) = session_id else {
        return;
    };

    ensure_session_exists(session_provider, session_id, user_id).await;

    let tokens = tokens.unwrap_or(0);
    if let Err(e) = session_provider
        .increment_ai_usage(session_id, tokens, cost_microdollars)
        .await
    {
        error!(
            error = %e,
            session_id = %session_id,
            tokens = tokens,
            cost_microdollars = cost_microdollars,
            "Failed to update session AI usage"
        );
    }
}

async fn ensure_session_exists(
    session_provider: &dyn AiSessionProvider,
    session_id: &SessionId,
    user_id: &UserId,
) {
    let exists = session_provider
        .session_exists(session_id)
        .await
        .map_err(|e| {
            error!(error = %e, session_id = %session_id, "Failed to check session existence");
            e
        })
        .unwrap_or(false);

    if exists {
        return;
    }

    let jwt_expiration = systemprompt_models::Config::get()
        .map(|c| c.jwt_access_token_expiration)
        .map_err(|e| {
            error!(error = %e, "Failed to get config for JWT expiration, using default 3600s");
            e
        })
        .unwrap_or(3600);
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration);

    let params = CreateAiSessionParams {
        session_id,
        user_id: Some(user_id),
        session_source: SessionSource::Api,
        expires_at,
    };

    if let Err(e) = session_provider.create_session(params).await {
        error!(
            error = %e,
            session_id = %session_id,
            user_id = %user_id,
            "Failed to create session for AI usage tracking"
        );
    }
}
