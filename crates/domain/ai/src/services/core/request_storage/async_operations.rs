use crate::models::AiRequestRecord;
use crate::repository::{AiRequestRepository, InsertToolCallParams};
use systemprompt_core_analytics::{CreateSessionParams, SessionRepository};
use systemprompt_identifiers::{AiRequestId, SessionId, UserId};

use super::record_builder::{MessageData, ToolCallData};

pub async fn store_request_async(
    repo: &AiRequestRepository,
    record: &AiRequestRecord,
) -> Option<AiRequestId> {
    repo.insert(record).await.ok()
}

pub async fn store_messages_async(
    repo: &AiRequestRepository,
    db_id: &AiRequestId,
    messages: Vec<MessageData>,
) {
    for message in messages {
        let _ = repo
            .insert_message(db_id, &message.role, &message.content, message.sequence)
            .await;
    }
}

pub async fn store_tool_calls_async(
    repo: &AiRequestRepository,
    db_id: &AiRequestId,
    tool_calls: Vec<ToolCallData>,
) {
    for tool_call in tool_calls {
        let _ = repo
            .insert_tool_call(InsertToolCallParams {
                request_id: db_id,
                ai_tool_call_id: &tool_call.ai_tool_call_id,
                tool_name: &tool_call.tool_name,
                tool_input: &tool_call.tool_input,
                sequence_number: tool_call.sequence,
            })
            .await;
    }
}

pub async fn update_session_usage_async(
    session_repo: &SessionRepository,
    user_id: &UserId,
    session_id: Option<&SessionId>,
    tokens: Option<i32>,
    cost_cents: i32,
) {
    if user_id.as_str() == "system" {
        return;
    }

    let Some(session_id) = session_id else {
        return;
    };

    ensure_session_exists(session_repo, session_id, user_id).await;

    let tokens = tokens.unwrap_or(0);
    let _ = session_repo
        .increment_ai_usage(session_id, tokens, cost_cents)
        .await;
}

async fn ensure_session_exists(
    session_repo: &SessionRepository,
    session_id: &SessionId,
    user_id: &UserId,
) {
    let exists = session_repo.exists(session_id).await.unwrap_or(false);

    if exists {
        return;
    }

    let jwt_expiration = systemprompt_models::Config::get()
        .map(|c| c.jwt_access_token_expiration)
        .unwrap_or(3600);
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration);

    let params = CreateSessionParams {
        session_id,
        user_id: Some(user_id),
        fingerprint_hash: None,
        ip_address: None,
        user_agent: None,
        device_type: None,
        browser: None,
        os: None,
        country: None,
        region: None,
        city: None,
        preferred_locale: None,
        referrer_source: None,
        referrer_url: None,
        landing_page: None,
        entry_url: None,
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        is_bot: false,
        expires_at,
    };

    let _ = session_repo.create_session(&params).await;
}
