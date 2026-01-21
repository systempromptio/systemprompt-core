use crate::repository::OAuthRepository;
use crate::services::verify_client_secret;
use anyhow::Result;

/// Validates client credentials with timing-attack resistant comparison.
///
/// This function performs constant-time secret comparison to prevent timing attacks.
/// When auth_method is "none", no secret verification is performed.
pub async fn validate_client_credentials(
    repo: &OAuthRepository,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<()> {
    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    let auth_method = client.token_endpoint_auth_method.as_str();

    match auth_method {
        "none" => Ok(()),
        _ => {
            // Always perform a dummy hash verification to prevent timing attacks
            // that could reveal whether a client has a secret configured
            let dummy_hash = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4.VTtYA/7E/fxXwK";

            let (hash_to_verify, secret_to_verify) = match (&client.client_secret_hash, client_secret) {
                (Some(hash), Some(secret)) => (hash.as_str(), secret),
                (Some(_hash), None) => {
                    // Do a dummy verification to maintain constant time
                    let _ = verify_client_secret("dummy_secret", dummy_hash);
                    return Err(anyhow::anyhow!("Client secret required"));
                },
                (None, Some(_secret)) => {
                    // Do a dummy verification to maintain constant time
                    let _ = verify_client_secret("dummy_secret", dummy_hash);
                    return Err(anyhow::anyhow!("Client has no secret hash configured"));
                },
                (None, None) => {
                    // Do a dummy verification to maintain constant time
                    let _ = verify_client_secret("dummy_secret", dummy_hash);
                    return Err(anyhow::anyhow!("Client secret required"));
                },
            };

            if !verify_client_secret(secret_to_verify, hash_to_verify)? {
                return Err(anyhow::anyhow!("Invalid client secret"));
            }

            Ok(())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests would require mocking the repository
    // These are placeholder tests demonstrating the expected behavior

    #[test]
    fn test_dummy_hash_is_valid_bcrypt() {
        // Ensure our dummy hash is a valid bcrypt hash that can be verified
        let dummy_hash = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4.VTtYA/7E/fxXwK";
        // This should not panic
        let result = verify_client_secret("any_value", dummy_hash);
        assert!(result.is_ok());
    }
}
