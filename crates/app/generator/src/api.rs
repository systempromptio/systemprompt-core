use anyhow::{anyhow, Result};
use serde_json::Value;

pub async fn fetch_content_from_api(api_url: &str, source_id: &str) -> Result<Vec<Value>> {
    let url = format!("{api_url}/api/v1/content/{source_id}");

    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to connect to {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch {}: {} {}",
            source_id,
            response.status(),
            response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string())
        ));
    }

    let items: Vec<Value> = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response from {}: {}", source_id, e))?;

    Ok(items)
}
