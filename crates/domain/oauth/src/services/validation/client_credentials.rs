use crate::repository::OAuthRepository;
use crate::services::verify_client_secret;
use anyhow::Result;
use systemprompt_identifiers::ClientId;

const TIMING_SAFE_DUMMY_HASH: &str = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4.VTtYA/7E/fxXwK";

pub async fn validate_client_credentials(
    repo: &OAuthRepository,
    client_id: &ClientId,
    client_secret: Option<&str>,
) -> Result<()> {
    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    verify_client_authentication(
        client.token_endpoint_auth_method.as_str(),
        client.client_secret_hash.as_deref(),
        client_secret,
    )
}

pub fn verify_client_authentication(
    auth_method: &str,
    secret_hash: Option<&str>,
    client_secret: Option<&str>,
) -> Result<()> {
    if auth_method == "none" {
        return Ok(());
    }

    let (hash_to_verify, secret_to_verify) = match (secret_hash, client_secret) {
        (Some(hash), Some(secret)) => (hash, secret),
        (Some(_hash), None) => {
            perform_timing_safe_dummy_verification();
            return Err(anyhow::anyhow!("Client secret required"));
        },
        (None, Some(_secret)) => {
            perform_timing_safe_dummy_verification();
            return Err(anyhow::anyhow!("Client has no secret hash configured"));
        },
        (None, None) => {
            perform_timing_safe_dummy_verification();
            return Err(anyhow::anyhow!("Client secret required"));
        },
    };

    if !verify_client_secret(secret_to_verify, hash_to_verify)? {
        return Err(anyhow::anyhow!("Invalid client secret"));
    }

    Ok(())
}

fn perform_timing_safe_dummy_verification() {
    if let Err(e) = verify_client_secret("dummy_secret", TIMING_SAFE_DUMMY_HASH) {
        tracing::debug!(error = %e, "Timing-safe dummy verification encountered error");
    }
}
