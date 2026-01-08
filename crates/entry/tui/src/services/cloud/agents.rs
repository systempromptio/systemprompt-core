use anyhow::Result;
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionToken};
use systemprompt_models::AgentCard;

pub async fn fetch_agents(api_url: &str, token: &SessionToken) -> Result<Vec<AgentCard>> {
    let client = SystempromptClient::new(api_url)?.with_token(JwtToken::new(token.as_str()));
    client
        .list_agents()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch agents: {}", e))
}
