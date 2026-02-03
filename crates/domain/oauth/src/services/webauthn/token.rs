use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

const TOKEN_PREFIX: &str = "sp_wst_";

#[must_use]
pub fn generate_setup_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);

    let raw_token = format!("{}{}", TOKEN_PREFIX, URL_SAFE_NO_PAD.encode(bytes));
    let hash = hash_token(&raw_token);

    (raw_token, hash)
}

#[must_use]
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    URL_SAFE_NO_PAD.encode(result)
}

pub fn validate_token_format(token: &str) -> Result<()> {
    let Some(encoded) = token.strip_prefix(TOKEN_PREFIX) else {
        anyhow::bail!("Invalid token format: missing prefix");
    };
    URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| anyhow::anyhow!("Invalid token format: invalid encoding"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let (token, hash) = generate_setup_token();
        assert!(token.starts_with(TOKEN_PREFIX));
        assert!(!hash.is_empty());

        assert_eq!(hash_token(&token), hash);
    }

    #[test]
    fn test_validate_token_format() {
        let (token, _) = generate_setup_token();
        assert!(validate_token_format(&token).is_ok());

        assert!(validate_token_format("invalid").is_err());
        assert!(validate_token_format("sp_wst_!!!invalid!!!").is_err());
    }
}
