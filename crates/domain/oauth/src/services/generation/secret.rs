//! Opaque-token and client-secret generation and hashing.

use rand::distr::Alphanumeric;
use rand::{RngExt, rng};

use crate::error::OauthResult as Result;

pub fn generate_secure_token(prefix: &str) -> String {
    let mut rng = rng();
    let token: String = (0..32)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect();

    format!("{prefix}_{token}")
}

pub fn generate_client_secret() -> String {
    let mut rng = rng();
    let secret: String = (0..64)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect();

    format!("secret_{secret}")
}

pub fn generate_access_token_jti() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn hash_client_secret(secret: &str) -> Result<String> {
    use bcrypt::{DEFAULT_COST, hash};
    Ok(hash(secret, DEFAULT_COST)?)
}

pub fn verify_client_secret(secret: &str, hash: &str) -> Result<bool> {
    use bcrypt::verify;
    Ok(verify(secret, hash)?)
}
