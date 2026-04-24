use chrono::{DateTime, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ApiKeyId, UserId};

use crate::error::{Result, UserError};
use crate::models::{NewApiKey, UserApiKey};
use crate::repository::{CreateApiKeyParams, UserRepository};

pub const API_KEY_PREFIX: &str = "sp-live-";
const SECRET_BYTES: usize = 32;
const PREFIX_ID_BYTES: usize = 6;

#[derive(Debug, Clone)]
pub struct IssueApiKeyParams<'a> {
    pub user_id: &'a UserId,
    pub name: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ApiKeyService {
    repository: UserRepository,
}

impl ApiKeyService {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        Ok(Self {
            repository: UserRepository::new(db)?,
        })
    }

    pub async fn issue(&self, params: IssueApiKeyParams<'_>) -> Result<NewApiKey> {
        let trimmed = params.name.trim();
        if trimmed.is_empty() {
            return Err(UserError::Validation(
                "api key name must not be empty".into(),
            ));
        }

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
                expires_at: params.expires_at,
            })
            .await?;

        Ok(NewApiKey { record, secret })
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

        if !record.is_active(Utc::now()) {
            return Ok(None);
        }

        let presented_hash = hash_secret(presented_secret);
        if presented_hash
            .as_bytes()
            .ct_eq(record.key_hash.as_bytes())
            .into()
        {
            self.repository.touch_api_key_usage(&record.id).await?;
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
