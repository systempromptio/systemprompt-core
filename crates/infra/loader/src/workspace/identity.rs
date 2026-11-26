//! Replica identity secrets: the values every node must share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use rand::distr::Alphanumeric;
use rand::{RngExt, rng};
use serde::Serialize;
use systemprompt_config::{generate_master_key, generate_seed};
use systemprompt_security::keys::{KeyError, RsaSigningKey};

#[derive(Debug, Clone, Serialize)]
pub struct IdentityBundle {
    pub oauth_at_rest_pepper: String,
    pub manifest_signing_secret_seed: String,
    pub signing_key_pem: String,
    pub signing_kid: String,
    pub encryption_master_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("RSA keypair generation failed")]
    KeyGeneration(#[source] KeyError),
    #[error("PKCS#8 PEM encoding failed")]
    PemEncoding(#[source] KeyError),
}

pub fn generate_identity() -> Result<IdentityBundle, IdentityError> {
    let key = RsaSigningKey::generate().map_err(IdentityError::KeyGeneration)?;
    let pem = key.to_pkcs8_pem().map_err(IdentityError::PemEncoding)?;
    let standard = base64::engine::general_purpose::STANDARD;
    Ok(IdentityBundle {
        oauth_at_rest_pepper: generate_oauth_at_rest_pepper(),
        manifest_signing_secret_seed: standard.encode(generate_seed()),
        signing_key_pem: standard.encode(pem.as_bytes()),
        signing_kid: key.kid().to_owned(),
        encryption_master_key: generate_master_key(),
    })
}

pub fn generate_oauth_at_rest_pepper() -> String {
    let mut rng = rng();
    (0..64)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
}
