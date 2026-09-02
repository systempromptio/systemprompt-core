//! Replica identity secrets: the three values every node must share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use base64::Engine;
use serde::Serialize;
use systemprompt_config::generate_seed;
use systemprompt_security::keys::RsaSigningKey;

use super::profile::generate_oauth_at_rest_pepper;

#[derive(Debug, Clone, Serialize)]
pub struct IdentityBundle {
    pub oauth_at_rest_pepper: String,
    pub manifest_signing_secret_seed: String,
    pub signing_key_pem: String,
    pub signing_kid: String,
}

pub fn generate_identity() -> Result<IdentityBundle> {
    let key = RsaSigningKey::generate().context("RSA keypair generation failed")?;
    let pem = key.to_pkcs8_pem().context("PKCS#8 PEM encoding failed")?;
    let standard = base64::engine::general_purpose::STANDARD;
    Ok(IdentityBundle {
        oauth_at_rest_pepper: generate_oauth_at_rest_pepper(),
        manifest_signing_secret_seed: standard.encode(generate_seed()),
        signing_key_pem: standard.encode(pem.as_bytes()),
        signing_kid: key.kid().to_owned(),
    })
}
