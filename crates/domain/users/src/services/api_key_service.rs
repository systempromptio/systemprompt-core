//! API-key minting: prefixed secrets with stored hashes.
//!
//! A key's `purpose` is bound by its issuer (for example `workflow:<id>`) and
//! is `None` for a person's own key.
//!
//! Every key passes through [`ApiKeyLifetime`]: with a ceiling configured, an
//! omitted expiry becomes `now + max` and an expiry beyond it is refused with
//! [`UserError::ApiKeyLifetimeExceeded`]. The ceiling also holds for keys
//! issued before it was configured: [`ApiKeyService::verify`] refuses a key
//! older than the ceiling, whatever its stored expiry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::attribution::ScopeBinding;

use crate::error::{Result, UserError};
use crate::models::{ApiKeyKind, ApiKeyLimits, NewApiKey, UserApiKey};
use crate::repository::{CreateApiKeyParams, UserRepository};

pub const API_KEY_PREFIX: &str = "sp-live-";
const SECRET_BYTES: usize = 32;
const PREFIX_ID_BYTES: usize = 6;
const USAGE_TOUCH_INTERVAL: chrono::TimeDelta = chrono::TimeDelta::seconds(60);

#[derive(Debug, Clone)]
pub struct IssueApiKeyParams<'a> {
    pub user_id: &'a UserId,
    pub name: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
    pub limits: &'a ApiKeyLimits,
    pub scopes: &'a [ScopeBinding],
    pub purpose: Option<&'a str>,
    pub kind: ApiKeyKind,
}

/// The lifetime ceiling applied to every issued key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApiKeyLifetime {
    max_days: Option<u32>,
}

impl ApiKeyLifetime {
    pub const fn unbounded() -> Self {
        Self { max_days: None }
    }

    pub const fn from_max_days(max_days: Option<u32>) -> Self {
        Self { max_days }
    }

    pub const fn max_days(self) -> Option<u32> {
        self.max_days
    }

    pub fn bound(
        self,
        requested: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>> {
        let Some(max_days) = self.max_days else {
            return Ok(requested);
        };
        let ceiling = now + chrono::TimeDelta::days(i64::from(max_days));
        match requested {
            None => Ok(Some(ceiling)),
            Some(at) if at > ceiling => Err(UserError::ApiKeyLifetimeExceeded {
                requested: at,
                max_days,
            }),
            Some(at) => Ok(Some(at)),
        }
    }

    pub fn admits(self, created_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        let Some(max_days) = self.max_days else {
            return true;
        };
        created_at.is_some_and(|at| now < at + chrono::TimeDelta::days(i64::from(max_days)))
    }
}

/// Outcome of [`ApiKeyService::ensure`]: the reused live key, or a fresh one
/// whose secret is available exactly once.
#[derive(Debug)]
pub enum EnsuredApiKey {
    Existing(UserApiKey),
    Issued(NewApiKey),
}

#[derive(Debug, Clone)]
pub struct ApiKeyService {
    repository: Arc<UserRepository>,
    lifetime: ApiKeyLifetime,
}

impl ApiKeyService {
    pub const fn new(repository: Arc<UserRepository>, lifetime: ApiKeyLifetime) -> Self {
        Self {
            repository,
            lifetime,
        }
    }

    pub async fn issue(&self, params: IssueApiKeyParams<'_>) -> Result<NewApiKey> {
        let trimmed = params.name.trim();
        if trimmed.is_empty() {
            return Err(UserError::Validation(
                "api key name must not be empty".into(),
            ));
        }

        validate_limits(params.limits)?;
        validate_scopes(params.scopes)?;
        let expires_at = self.lifetime.bound(params.expires_at, Utc::now())?;

        let id = ApiKeyId::generate();
        let (secret, key_prefix, key_hash) = generate_secret();

        let record = self
            .repository
            .create_api_key(CreateApiKeyParams {
                id: &id,
                user_id: params.user_id,
                name: trimmed,
                key_prefix: &key_prefix,
                key_hash: &key_hash,
                expires_at,
                limits: params.limits,
                scopes: params.scopes,
                purpose: params.purpose,
                kind: params.kind,
            })
            .await?;

        Ok(NewApiKey { record, secret })
    }

    pub async fn ensure(
        &self,
        params: IssueApiKeyParams<'_>,
        renew_before: chrono::TimeDelta,
    ) -> Result<EnsuredApiKey> {
        let name = params.name.trim();
        let now = Utc::now();
        let renew_at = now + renew_before;
        let existing = self
            .repository
            .list_api_keys_for_user(params.user_id)
            .await?
            .into_iter()
            .find(|key| {
                key.name == name
                    && key.is_active(now)
                    && self.lifetime.admits(key.created_at, renew_at)
                    && key.expires_at.is_none_or(|at| at > renew_at)
            });
        match existing {
            Some(key) => Ok(EnsuredApiKey::Existing(key)),
            None => self.issue(params).await.map(EnsuredApiKey::Issued),
        }
    }

    pub async fn verify(&self, presented_secret: &str) -> Result<Option<UserApiKey>> {
        let Some(key_prefix) = extract_prefix(presented_secret) else {
            return Ok(None);
        };

        let Some(record) = self
            .repository
            .find_active_api_key_by_prefix(&key_prefix)
            .await?
        else {
            return Ok(None);
        };

        let now = Utc::now();
        if !record.is_active(now) || !self.lifetime.admits(record.created_at, now) {
            return Ok(None);
        }

        let presented_hash = hash_secret(presented_secret);
        if presented_hash
            .as_bytes()
            .ct_eq(record.key_hash.as_bytes())
            .into()
        {
            if usage_is_stale(record.last_used_at, Utc::now()) {
                self.repository.touch_api_key_usage(&record.id).await?;
            }
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub async fn list_for_user(&self, user_id: &UserId) -> Result<Vec<UserApiKey>> {
        self.repository.list_api_keys_for_user(user_id).await
    }

    pub async fn revoke(&self, id: &ApiKeyId, user_id: &UserId) -> Result<bool> {
        self.repository.revoke_api_key(id, user_id).await
    }
}

fn validate_limits(limits: &ApiKeyLimits) -> Result<()> {
    if limits
        .model_allowlist
        .as_ref()
        .is_some_and(|models| models.is_empty() || models.iter().any(|m| m.trim().is_empty()))
    {
        return Err(UserError::Validation(
            "model_allowlist must name at least one model, or be omitted".into(),
        ));
    }
    if limits.budget_microdollars.is_some_and(|b| b < 0)
        || limits.max_requests.is_some_and(|m| m < 0)
    {
        return Err(UserError::Validation(
            "budget_microdollars and max_requests must not be negative".into(),
        ));
    }
    let ceiling = limits.budget_microdollars.is_some() || limits.max_requests.is_some();
    if ceiling && limits.request_window_seconds.is_none_or(|w| w <= 0) {
        return Err(UserError::Validation(
            "budget_microdollars and max_requests need a positive request_window_seconds".into(),
        ));
    }
    Ok(())
}

fn validate_scopes(scopes: &[ScopeBinding]) -> Result<()> {
    for (index, scope) in scopes.iter().enumerate() {
        if scope.value.trim().is_empty() {
            return Err(UserError::Validation(format!(
                "scope {} must bind a non-empty value",
                scope.dimension
            )));
        }
        if scopes[..index]
            .iter()
            .any(|s| s.dimension == scope.dimension)
        {
            return Err(UserError::Validation(format!(
                "scope {} is bound more than once",
                scope.dimension
            )));
        }
    }
    Ok(())
}

fn generate_secret() -> (String, String, String) {
    let mut raw = [0u8; SECRET_BYTES];
    rand::rng().fill_bytes(&mut raw);
    let encoded = hex::encode(raw);
    let key_prefix = format!("{API_KEY_PREFIX}{}", &encoded[..PREFIX_ID_BYTES * 2]);
    let secret = format!("{key_prefix}.{}", &encoded[PREFIX_ID_BYTES * 2..]);
    let key_hash = hash_secret(&secret);
    (secret, key_prefix, key_hash)
}

fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

fn extract_prefix(presented: &str) -> Option<String> {
    if !presented.starts_with(API_KEY_PREFIX) {
        return None;
    }
    let dot = presented.find('.')?;
    Some(presented[..dot].to_string())
}

fn usage_is_stale(last_used_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    last_used_at.is_none_or(|at| now - at >= USAGE_TOUCH_INTERVAL)
}
