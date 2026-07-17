//! Security-section mutations behind the `admin config security` surface.
//!
//! [`SecurityConfigService`] edits the profile's [`SecurityConfig`]: the JWT
//! issuer, token expiries, the allowed resource audiences, and the federated
//! trusted-issuer list. Every mutation yields a [`SecurityChange`] record
//! (field, old value, new value, message) for rendering or auditing. Audience
//! updates always re-seed from
//! [`default_resource_audiences`], so the gateway-required audiences can never
//! be removed. Callers revalidate and persist the profile afterwards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::profile::{SecurityConfig, TrustedIssuer, default_resource_audiences};

use crate::error::{ConfigError, ConfigResult};

#[derive(Debug, Clone)]
pub struct SecurityChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityUpdate {
    pub jwt_issuer: Option<String>,
    pub access_token_expiration: Option<i64>,
    pub refresh_token_expiration: Option<i64>,
    pub resource_audiences: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct SecurityConfigService;

impl SecurityConfigService {
    pub fn apply_update(
        security: &mut SecurityConfig,
        update: &SecurityUpdate,
    ) -> ConfigResult<Vec<SecurityChange>> {
        let mut changes = Vec::new();

        if let Some(ref issuer) = update.jwt_issuer {
            let old = security.issuer.clone();
            security.issuer.clone_from(issuer);
            changes.push(SecurityChange {
                field: "jwt_issuer".to_owned(),
                old_value: old,
                new_value: issuer.clone(),
                message: format!("Updated JWT issuer to {}", issuer),
            });
        }

        if let Some(expiry) = update.access_token_expiration {
            if expiry <= 0 {
                return Err(ConfigError::NonPositiveAccessTokenExpiry);
            }
            let old = security.access_token_expiration;
            security.access_token_expiration = expiry;
            changes.push(SecurityChange {
                field: "access_token_expiration".to_owned(),
                old_value: old.to_string(),
                new_value: expiry.to_string(),
                message: format!("Updated access token expiry to {} seconds", expiry),
            });
        }

        if let Some(expiry) = update.refresh_token_expiration {
            if expiry <= 0 {
                return Err(ConfigError::NonPositiveRefreshTokenExpiry);
            }
            let old = security.refresh_token_expiration;
            security.refresh_token_expiration = expiry;
            changes.push(SecurityChange {
                field: "refresh_token_expiration".to_owned(),
                old_value: old.to_string(),
                new_value: expiry.to_string(),
                message: format!("Updated refresh token expiry to {} seconds", expiry),
            });
        }

        if !update.resource_audiences.is_empty() {
            changes.push(Self::merge_resource_audiences(
                security,
                &update.resource_audiences,
            ));
        }

        Ok(changes)
    }

    fn merge_resource_audiences(
        security: &mut SecurityConfig,
        requested: &[String],
    ) -> SecurityChange {
        let old = security.allowed_resource_audiences.join(",");
        let mut merged = default_resource_audiences();
        for aud in requested {
            if !merged.contains(aud) {
                merged.push(aud.clone());
            }
        }
        security.allowed_resource_audiences.clone_from(&merged);
        SecurityChange {
            field: "allowed_resource_audiences".to_owned(),
            old_value: old,
            new_value: merged.join(","),
            message: "Updated allowed resource audiences".to_owned(),
        }
    }

    pub fn upsert_trusted_issuer(
        security: &mut SecurityConfig,
        entry: TrustedIssuer,
    ) -> SecurityChange {
        security
            .trusted_issuers
            .retain(|t| t.issuer != entry.issuer);
        let change = SecurityChange {
            field: "trusted_issuers".to_owned(),
            old_value: String::new(),
            new_value: entry.issuer.clone(),
            message: format!("Added trusted issuer {}", entry.issuer),
        };
        security.trusted_issuers.push(entry);
        change
    }

    pub fn remove_trusted_issuer(
        security: &mut SecurityConfig,
        issuer: &str,
    ) -> ConfigResult<SecurityChange> {
        let before = security.trusted_issuers.len();
        security.trusted_issuers.retain(|t| t.issuer != issuer);
        if security.trusted_issuers.len() == before {
            return Err(ConfigError::TrustedIssuerNotFound {
                issuer: issuer.to_owned(),
            });
        }
        Ok(SecurityChange {
            field: "trusted_issuers".to_owned(),
            old_value: issuer.to_owned(),
            new_value: String::new(),
            message: format!("Removed trusted issuer {}", issuer),
        })
    }
}
