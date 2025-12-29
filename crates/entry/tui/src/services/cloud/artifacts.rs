use anyhow::Result;
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;

pub async fn list_all_artifacts(
    api_url: &str,
    token: &JwtToken,
    limit: Option<u32>,
) -> Result<Vec<systemprompt_models::A2aArtifact>> {
    let client = SystempromptClient::new(api_url)?.with_token(token.clone());
    let artifacts_json = client.list_all_artifacts(limit).await?;

    let artifacts: Vec<systemprompt_models::A2aArtifact> = artifacts_json
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect();

    Ok(artifacts)
}
