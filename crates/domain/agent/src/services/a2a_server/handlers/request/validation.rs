use crate::models::a2a::A2aJsonRpcRequest;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use systemprompt_identifiers::UserId;

pub async fn validate_message_context(
    message: &crate::models::a2a::Message,
    user_id: Option<&str>,
    db_pool: &systemprompt_database::DbPool,
) -> Result<(), String> {
    let context_id = &message.context_id;

    let user_id_str =
        user_id.ok_or_else(|| "User authentication required for message processing".to_string())?;

    if user_id_str == "missing-user-id" || user_id_str.is_empty() {
        return Err(
            "Authentication required: x-user-id header must be set by API proxy after JWT \
             validation"
                .to_string(),
        );
    }

    let user_id = UserId::new(user_id_str.to_string());
    let context_repo = crate::repository::ContextRepository::new(db_pool.clone());
    context_repo
        .validate_context_ownership(context_id, &user_id)
        .await
        .map_err(|e| format!("Context validation failed: {e}"))?;

    Ok(())
}

pub async fn should_require_oauth(_request: &A2aJsonRpcRequest, state: &AgentHandlerState) -> bool {
    let config = state.config.read().await;
    config.oauth.required
}
