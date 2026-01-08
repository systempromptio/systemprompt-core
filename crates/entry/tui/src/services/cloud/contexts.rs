use anyhow::Result;
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{ContextId, JwtToken, SessionToken};
use systemprompt_models::UserContextWithStats;

fn to_jwt(token: &SessionToken) -> JwtToken {
    JwtToken::new(token.as_str())
}

pub async fn fetch_or_create_context(api_url: &str, token: &SessionToken) -> Result<ContextId> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    client
        .fetch_or_create_context()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch/create context: {}", e))
}

pub async fn create_context(api_url: &str, token: &SessionToken) -> Result<ContextId> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    let context = client
        .create_context_auto_name()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create context: {}", e))?;
    Ok(context.context_id)
}

pub async fn list_contexts(
    api_url: &str,
    token: &SessionToken,
) -> Result<Vec<UserContextWithStats>> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    client
        .list_contexts()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list contexts: {}", e))
}

pub async fn update_context_name(
    api_url: &str,
    token: &SessionToken,
    context_id: &str,
    name: &str,
) -> Result<()> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    client
        .update_context_name(context_id, name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update context name: {}", e))
}

pub async fn delete_context(api_url: &str, token: &SessionToken, context_id: &str) -> Result<()> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    client
        .delete_context(context_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete context: {}", e))
}

pub async fn create_context_with_name(
    api_url: &str,
    token: &SessionToken,
    name: &str,
) -> Result<String> {
    let client = SystempromptClient::new(api_url)?.with_token(to_jwt(token));
    let context = client
        .create_context(Some(name))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create context: {}", e))?;
    Ok(context.context_id.to_string())
}
