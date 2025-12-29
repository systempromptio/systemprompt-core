use anyhow::Result;
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;
use systemprompt_models::AgentCard;

pub async fn discover_agents_with_token(api_url: &str, token: &JwtToken) -> Result<Vec<AgentCard>> {
    tracing::debug!("Discovering agents from registry: {}", api_url);

    let client = SystempromptClient::new(api_url)?.with_token(token.clone());

    match client.list_agents().await {
        Ok(cards) => {
            tracing::info!(
                "Successfully discovered {} agents from registry",
                cards.len()
            );
            Ok(cards)
        },
        Err(e) => {
            tracing::error!("Failed to fetch agents from registry: {}", e);
            Err(anyhow::anyhow!(
                "Failed to fetch agents from registry: {}",
                e
            ))
        },
    }
}
