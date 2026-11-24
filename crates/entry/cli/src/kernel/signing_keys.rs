//! The kernel-held deployment signing-key handle a
//! `KernelGrant::SigningKeys` command reads through.
//!
//! [`KernelSigningKeys`] reads the token keys and the manifest signing seed
//! from the active profile's boot context, writes a rotation back to that
//! profile's secrets file, mints a plugin-scoped hook token under the
//! profile's issuer, and returns only key ids, expiry instants, the manifest
//! public key and the minted token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use base64::Engine;
use chrono::{Duration, Utc};
use ed25519_dalek::SigningKey;
use systemprompt_config::{SecretsService, SigningKeyState};
use systemprompt_extension::cli::{
    CommandError, DeploymentSigningKeys, MintedPluginToken, PluginTokenGrant, RetiredTokenKey,
    TokenKeyPromotion, TokenKeySet,
};
use systemprompt_loader::BootContext;
use systemprompt_manifest::profile::SecurityConfig;
use systemprompt_models::auth::{TokenAuthority, UserJwtRequest};
use systemprompt_security::DeploymentTokenAuthority;
use systemprompt_security::jwt::{cut_off_by, rotation_overlap, signed_token_lifetimes};
use systemprompt_security::keys::{RotationMode, RsaSigningKey, promote, stage};

/// The deployment signing state of the profile the CLI booted.
pub struct KernelSigningKeys {
    boot: Arc<BootContext>,
    security: SecurityConfig,
}

impl std::fmt::Debug for KernelSigningKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelSigningKeys").finish_non_exhaustive()
    }
}

impl KernelSigningKeys {
    #[must_use]
    pub const fn new(boot: Arc<BootContext>, security: SecurityConfig) -> Self {
        Self { boot, security }
    }

    fn current(&self) -> Result<SigningKeyState> {
        let keys = crate::session::api::load_signing_keys(&self.boot)?;
        let secrets = self.boot.secrets();
        Ok(SigningKeyState {
            active_private_pem: keys
                .signing_key()
                .to_pkcs8_pem()
                .context("Failed to encode the active signing key")?,
            staged_private_pem: secrets
                .signing_key_staged_pem()
                .context("Failed to read the staged signing key")?,
            previous: secrets
                .signing_keys_previous()
                .context("Failed to read the previous signing keys")?,
        })
    }

    fn persist(&self, state: &SigningKeyState) -> Result<()> {
        SecretsService::persist_signing_key_state(self.boot.loaded_profile(), state)
            .context("Failed to write the signing keys to the secrets file")
    }

    fn promote_from(
        &self,
        state: &SigningKeyState,
        promotion: TokenKeyPromotion,
    ) -> Result<TokenKeySet> {
        let (mode, cut_off) = match promotion {
            TokenKeyPromotion::Revoke => (RotationMode::Revoke, Vec::new()),
            TokenKeyPromotion::Overlap { secs, force } => {
                let requested = secs
                    .map(|secs| {
                        Duration::try_seconds(secs)
                            .with_context(|| format!("--overlap-secs {secs} is out of range"))
                    })
                    .transpose()?;
                let overlap = rotation_overlap(&self.security, requested, force)?;
                let cut_off = cut_off_by(&signed_token_lifetimes(&self.security), overlap)
                    .into_iter()
                    .map(|cut| cut.kind.description().to_owned())
                    .collect();
                (RotationMode::Overlap(overlap), cut_off)
            },
        };
        let (_, next) =
            promote(state, mode, Utc::now()).context("Failed to promote the staged signing key")?;
        self.persist(&next)?;
        let mut set = key_set(&next)?;
        set.cut_off = cut_off;
        Ok(set)
    }

    fn manifest_seed(&self) -> Result<[u8; 32]> {
        Ok(self.boot.secrets().manifest_signing_secret_seed()?)
    }
}

fn key_set(state: &SigningKeyState) -> Result<TokenKeySet> {
    let kid =
        |pem: &str| -> Result<String> { Ok(RsaSigningKey::from_pkcs8_pem(pem)?.kid().to_owned()) };
    Ok(TokenKeySet {
        active_kid: kid(&state.active_private_pem)?,
        staged_kid: state.staged_private_pem.as_deref().map(kid).transpose()?,
        retired: state
            .previous
            .iter()
            .map(|entry| RetiredTokenKey {
                kid: entry.kid.clone(),
                expires_at: entry.expires_at,
            })
            .collect(),
        cut_off: Vec::new(),
    })
}

fn manifest_public_key(seed: &[u8; 32]) -> String {
    base64::engine::general_purpose::STANDARD
        .encode(SigningKey::from_bytes(seed).verifying_key().to_bytes())
}

impl DeploymentSigningKeys for KernelSigningKeys {
    fn token_keys(&self) -> Result<TokenKeySet, CommandError> {
        self.current()
            .and_then(|state| key_set(&state))
            .map_err(CommandError::new)
    }

    fn stage_token_key(&self) -> Result<TokenKeySet, CommandError> {
        (|| {
            let (_, next) = stage(&self.current()?).context("Failed to stage a signing key")?;
            self.persist(&next)?;
            key_set(&next)
        })()
        .map_err(CommandError::new)
    }

    fn promote_token_key(&self, promotion: TokenKeyPromotion) -> Result<TokenKeySet, CommandError> {
        self.current()
            .and_then(|state| self.promote_from(&state, promotion))
            .map_err(CommandError::new)
    }

    fn rotate_token_key(&self, promotion: TokenKeyPromotion) -> Result<TokenKeySet, CommandError> {
        (|| {
            let (_, staged) = stage(&self.current()?).context("Failed to stage a signing key")?;
            self.promote_from(&staged, promotion)
        })()
        .map_err(CommandError::new)
    }

    fn manifest_public_key(&self) -> Result<String, CommandError> {
        self.manifest_seed()
            .map(|seed| manifest_public_key(&seed))
            .map_err(CommandError::new)
    }

    fn rotate_manifest_key(&self) -> Result<String, CommandError> {
        SecretsService::rotate_manifest_signing_seed(self.boot.loaded_profile())
            .map(|seed| manifest_public_key(&seed))
            .map_err(CommandError::new)
    }

    fn mint_plugin_token(
        &self,
        grant: PluginTokenGrant,
    ) -> Result<MintedPluginToken, CommandError> {
        let keys = crate::session::api::load_signing_keys(&self.boot).map_err(CommandError::new)?;
        let authority = DeploymentTokenAuthority::new(keys, self.security.issuer.clone());
        let request = UserJwtRequest::plugin_hook(
            grant.user_id,
            grant.username,
            grant.email,
            grant.plugin_id,
            grant.duration_days,
            grant.session_id,
        );
        let token = authority
            .mint_user_jwt(&request)
            .map_err(CommandError::new)?;
        Ok(MintedPluginToken {
            token,
            jti: request.jti,
        })
    }
}
