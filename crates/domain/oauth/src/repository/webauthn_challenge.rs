//! `WebAuthn` ceremony state: challenges live in Postgres so a ceremony started
//! on one replica can finish on another.
//!
//! The account-link kind is single-flight per account. A live link challenge
//! for the same setup token is handed back to every subsequent start, because
//! a browser that has already completed `navigator.credentials.create()` for
//! one challenge must be able to finish with whichever start response its
//! client paired it with. The user row is locked for the reservation so
//! concurrent starts converge on one row instead of racing the insert.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{OauthError, OauthResult as Result};
use chrono::Utc;
use std::time::Duration;
use systemprompt_identifiers::{TokenId, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthnChallengeKind {
    Registration,
    Authentication,
    Verified,
    Link,
}

impl WebAuthnChallengeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registration => "registration",
            Self::Authentication => "authentication",
            Self::Verified => "verified",
            Self::Link => "link",
        }
    }
}

#[derive(Debug)]
pub struct StoreChallengeParams<'a> {
    pub challenge: &'a str,
    pub kind: WebAuthnChallengeKind,
    pub user_id: Option<&'a UserId>,
    pub state: &'a serde_json::Value,
    pub oauth_state: Option<&'a str>,
    pub ttl: Duration,
}

#[derive(Debug, Clone)]
pub struct ConsumedChallenge {
    pub user_id: Option<UserId>,
    pub state: serde_json::Value,
    pub oauth_state: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ReserveLinkChallengeParams<'a> {
    pub user_id: &'a UserId,
    pub token_id: &'a TokenId,
    pub ttl: Duration,
    pub min_remaining: Duration,
}

#[derive(Debug, Clone)]
pub struct LinkChallengeReservation {
    pub challenge_id: String,
    pub state: serde_json::Value,
    pub reused: bool,
}

fn to_chrono(duration: Duration) -> Result<chrono::Duration> {
    chrono::Duration::from_std(duration)
        .map_err(|e| OauthError::Internal(format!("Challenge TTL out of range: {e}")))
}

impl crate::repository::OAuthRepository {
    pub async fn store_webauthn_challenge(&self, params: StoreChallengeParams<'_>) -> Result<()> {
        let expires_at = Utc::now() + to_chrono(params.ttl)?;
        let user_id = params.user_id.map(UserId::as_str);

        sqlx::query!(
            "INSERT INTO webauthn_challenges
             (challenge, user_id, challenge_type, session_state, oauth_state, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            params.challenge,
            user_id,
            params.kind.as_str(),
            params.state,
            params.oauth_state,
            expires_at
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    pub async fn reserve_link_challenge<F>(
        &self,
        params: ReserveLinkChallengeParams<'_>,
        mint: F,
    ) -> Result<LinkChallengeReservation>
    where
        F: FnOnce(&str) -> Result<serde_json::Value>,
    {
        let now = Utc::now();
        let usable_until = now + to_chrono(params.min_remaining)?;
        let expires_at = now + to_chrono(params.ttl)?;
        let user_id = params.user_id.as_str();
        let kind = WebAuthnChallengeKind::Link.as_str();

        let mut tx = self.write_pool_ref().begin().await?;

        sqlx::query!("SELECT id FROM users WHERE id = $1 FOR UPDATE", user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| OauthError::Internal("Link target user not found".to_owned()))?;

        let live = sqlx::query!(
            "SELECT challenge, session_state FROM webauthn_challenges
             WHERE user_id = $1 AND challenge_type = $2
               AND session_state->>'token_id' = $3 AND expires_at > $4",
            user_id,
            kind,
            params.token_id.as_str(),
            usable_until
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = live {
            tx.commit().await?;
            return Ok(LinkChallengeReservation {
                challenge_id: row.challenge,
                state: row.session_state.unwrap_or(serde_json::Value::Null),
                reused: true,
            });
        }

        sqlx::query!(
            "DELETE FROM webauthn_challenges WHERE user_id = $1 AND challenge_type = $2",
            user_id,
            kind
        )
        .execute(&mut *tx)
        .await?;

        let challenge_id = Uuid::new_v4().to_string();
        let state = mint(&challenge_id)?;

        sqlx::query!(
            "INSERT INTO webauthn_challenges
             (challenge, user_id, challenge_type, session_state, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
            challenge_id,
            user_id,
            kind,
            state,
            expires_at
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(LinkChallengeReservation {
            challenge_id,
            state,
            reused: false,
        })
    }

    pub async fn consume_webauthn_challenge(
        &self,
        challenge: &str,
        kind: WebAuthnChallengeKind,
    ) -> Result<Option<ConsumedChallenge>> {
        let row = sqlx::query!(
            "DELETE FROM webauthn_challenges
             WHERE challenge = $1 AND challenge_type = $2 AND expires_at > CURRENT_TIMESTAMP
             RETURNING user_id, session_state, oauth_state",
            challenge,
            kind.as_str()
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        Ok(row.map(|row| ConsumedChallenge {
            user_id: row.user_id.map(UserId::new),
            state: row.session_state.unwrap_or(serde_json::Value::Null),
            oauth_state: row.oauth_state,
        }))
    }

    pub async fn cleanup_expired_webauthn_challenges(&self) -> Result<u64> {
        let result =
            sqlx::query!("DELETE FROM webauthn_challenges WHERE expires_at <= CURRENT_TIMESTAMP")
                .execute(self.write_pool_ref())
                .await?;

        Ok(result.rows_affected())
    }
}
