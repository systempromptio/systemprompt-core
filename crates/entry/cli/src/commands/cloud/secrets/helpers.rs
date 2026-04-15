use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_cloud::{CloudApiClient, CloudCredentials, ProfilePath};
use systemprompt_identifiers::TenantId;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

pub fn get_tenant_id() -> Result<TenantId> {
    let profile =
        ProfileBootstrap::get().map_err(|_| anyhow::anyhow!("Profile not initialized"))?;

    let cloud = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cloud not configured in profile"))?;

    cloud
        .tenant_id
        .as_ref()
        .map(TenantId::new)
        .ok_or_else(|| anyhow::anyhow!("No tenant_id in profile. Create a cloud tenant first."))
}

pub fn get_tenant_and_secrets_path() -> Result<(TenantId, PathBuf)> {
    let tenant_id = get_tenant_id()?;

    let profile_path =
        ProfileBootstrap::get_path().map_err(|_| anyhow::anyhow!("Profile path not available"))?;

    let profile_dir = std::path::Path::new(profile_path)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        bail!(
            "secrets.json not found at {}. Create it first.",
            secrets_path.display()
        );
    }

    Ok((tenant_id, secrets_path))
}

pub fn load_secrets_json(path: &PathBuf) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).with_context(|| "Failed to parse secrets.json")?;

    let mut secrets = HashMap::new();

    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    secrets.insert(key.clone(), s.to_string());
                }
            }
        }
    }

    Ok(secrets)
}

pub fn map_secrets_to_env_vars(secrets: HashMap<String, String>) -> HashMap<String, String> {
    use systemprompt_cloud::constants::env_vars;

    let has_internal = secrets.contains_key("internal_database_url");

    let mut result: HashMap<String, String> = secrets
        .into_iter()
        .filter_map(|(k, v)| {
            let env_key = to_env_var_name(&k, has_internal)?;
            if env_vars::is_system_managed(&env_key) {
                tracing::warn!(key = %env_key, "Skipping system-managed variable from secrets.json");
                return None;
            }
            Some((env_key, v))
        })
        .collect();

    let custom_keys: Vec<String> = result
        .keys()
        .filter(|k| !is_standard_env_var(k))
        .cloned()
        .collect();

    if !custom_keys.is_empty() {
        result.insert(env_vars::CUSTOM_SECRETS.to_string(), custom_keys.join(","));
    }

    result
}

fn to_env_var_name(key: &str, has_internal_db_url: bool) -> Option<String> {
    match key {
        "gemini" => Some("GEMINI_API_KEY".to_string()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
        "openai" => Some("OPENAI_API_KEY".to_string()),
        "internal_database_url" => Some("DATABASE_URL".to_string()),
        "database_url" if has_internal_db_url => None,
        _ => Some(key.to_uppercase()),
    }
}

fn is_standard_env_var(key: &str) -> bool {
    matches!(
        key,
        "JWT_SECRET"
            | "DATABASE_URL"
            | "SYNC_TOKEN"
            | "GEMINI_API_KEY"
            | "ANTHROPIC_API_KEY"
            | "OPENAI_API_KEY"
            | "GITHUB_TOKEN"
    )
}

pub async fn sync_cloud_credentials(
    api_client: &CloudApiClient,
    tenant_id: &TenantId,
    creds: &CloudCredentials,
) -> Result<Vec<String>> {
    let mut secrets = HashMap::new();

    secrets.insert(
        "SYSTEMPROMPT_API_TOKEN".to_string(),
        creds.api_token.clone(),
    );

    secrets.insert(
        "SYSTEMPROMPT_USER_EMAIL".to_string(),
        creds.user_email.clone(),
    );

    secrets.insert("SYSTEMPROMPT_CLI_REMOTE".to_string(), "true".to_string());

    api_client.set_secrets(tenant_id.as_str(), secrets).await
}
