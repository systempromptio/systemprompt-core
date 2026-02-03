use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetupTokenPurpose {
    CredentialLink,
    Recovery,
}

impl SetupTokenPurpose {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CredentialLink => "credential_link",
            Self::Recovery => "recovery",
        }
    }
}

impl std::fmt::Display for SetupTokenPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SetupTokenPurpose {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "credential_link" => Ok(Self::CredentialLink),
            "recovery" => Ok(Self::Recovery),
            other => Err(anyhow::anyhow!("Invalid setup token purpose: {}", other)),
        }
    }
}

#[derive(Debug)]
pub struct CreateSetupTokenParams {
    pub user_id: String,
    pub token_hash: String,
    pub purpose: SetupTokenPurpose,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SetupTokenRecord {
    pub id: String,
    pub user_id: String,
    pub purpose: SetupTokenPurpose,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum TokenValidationResult {
    Valid(SetupTokenRecord),
    Expired,
    AlreadyUsed,
    NotFound,
}

impl crate::repository::OAuthRepository {
    pub async fn store_setup_token(&self, params: CreateSetupTokenParams) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query!(
            r#"
            INSERT INTO webauthn_setup_tokens (id, user_id, token_hash, purpose, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            id,
            params.user_id,
            params.token_hash,
            params.purpose.as_str(),
            params.expires_at
        )
        .execute(self.pool_ref())
        .await?;

        Ok(id)
    }

    pub async fn validate_setup_token(&self, token_hash: &str) -> Result<TokenValidationResult> {
        let row = sqlx::query!(
            r#"
            SELECT id, user_id, purpose, expires_at, used_at, created_at
            FROM webauthn_setup_tokens
            WHERE token_hash = $1
            "#,
            token_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;

        match row {
            None => Ok(TokenValidationResult::NotFound),
            Some(r) => {
                if r.used_at.is_some() {
                    return Ok(TokenValidationResult::AlreadyUsed);
                }
                if r.expires_at < Utc::now() {
                    return Ok(TokenValidationResult::Expired);
                }

                let purpose: SetupTokenPurpose = r.purpose.parse()?;

                Ok(TokenValidationResult::Valid(SetupTokenRecord {
                    id: r.id,
                    user_id: r.user_id,
                    purpose,
                    expires_at: r.expires_at,
                    created_at: r.created_at,
                }))
            },
        }
    }

    pub async fn consume_setup_token(&self, token_id: &str) -> Result<bool> {
        let rows_affected = sqlx::query!(
            r#"
            UPDATE webauthn_setup_tokens
            SET used_at = CURRENT_TIMESTAMP
            WHERE id = $1 AND used_at IS NULL
            "#,
            token_id
        )
        .execute(self.pool_ref())
        .await?
        .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn cleanup_expired_setup_tokens(&self) -> Result<u64> {
        let rows_affected = sqlx::query!(
            r#"
            DELETE FROM webauthn_setup_tokens
            WHERE (expires_at < CURRENT_TIMESTAMP - INTERVAL '24 hours')
               OR (used_at IS NOT NULL AND used_at < CURRENT_TIMESTAMP - INTERVAL '24 hours')
            "#
        )
        .execute(self.pool_ref())
        .await?
        .rows_affected();

        Ok(rows_affected)
    }

    pub async fn revoke_user_setup_tokens(&self, user_id: &str) -> Result<u64> {
        let rows_affected = sqlx::query!(
            r#"
            UPDATE webauthn_setup_tokens
            SET used_at = CURRENT_TIMESTAMP
            WHERE user_id = $1 AND used_at IS NULL
            "#,
            user_id
        )
        .execute(self.pool_ref())
        .await?
        .rows_affected();

        Ok(rows_affected)
    }
}
