//! Remembered authorization consent and the binding of a consent decision.
//!
//! A consent row records the scopes a person allowed an operator-declared
//! client. Both the read and the write join `oauth_clients` and require an
//! active row that still carries its declaration stamp, so a dynamically
//! registered client neither stores nor is covered by consent whatever the
//! caller asks.
//!
//! A consent token ties the Allow/Deny form to the exact authorization request
//! it was rendered for: it is the at-rest HMAC of the [`ConsentBinding`] and
//! an expiry, prefixed by that expiry. Nothing is stored; the server
//! recomputes the HMAC and compares it in constant time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use subtle::ConstantTimeEq;
use systemprompt_identifiers::{ClientId, SessionId, UserId};

use super::OAuthRepository;
use crate::error::OauthResult;

pub const CONSENT_TOKEN_TTL_SECS: i64 = 600;

const CONSENT_TOKEN_DOMAIN: &str = "authorize-consent-v1";

/// The authorization request a consent decision belongs to.
///
/// It names who is signed in, through which browser session, and the client,
/// redirect, scope, `state`, PKCE challenge and resource the code would be
/// minted for.
#[derive(Debug, Clone, Copy)]
pub struct ConsentBinding<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub client_id: &'a ClientId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub state: &'a str,
    pub code_challenge: &'a str,
    pub resource: Option<&'a str>,
}

impl ConsentBinding<'_> {
    fn material(&self, expires_at: i64) -> String {
        let resource = self
            .resource
            .map_or_else(|| "-".to_owned(), |resource| format!("+{resource}"));
        let expiry = expires_at.to_string();
        let fields = [
            CONSENT_TOKEN_DOMAIN,
            self.user_id.as_str(),
            self.session_id.as_str(),
            self.client_id.as_str(),
            self.redirect_uri,
            self.scope,
            self.state,
            self.code_challenge,
            resource.as_str(),
            expiry.as_str(),
        ];
        let mut material = String::new();
        for field in fields {
            material.push_str(&field.len().to_string());
            material.push(':');
            material.push_str(field);
        }
        material
    }
}

impl OAuthRepository {
    pub fn consent_token(
        &self,
        binding: &ConsentBinding<'_>,
        expires_at: i64,
    ) -> OauthResult<String> {
        let mac = self.hash_at_rest(&binding.material(expires_at))?;
        Ok(format!("{expires_at}.{mac}"))
    }

    pub fn consent_token_matches(
        &self,
        binding: &ConsentBinding<'_>,
        presented: &str,
        now: i64,
    ) -> OauthResult<bool> {
        let Some((expiry, mac)) = presented.split_once('.') else {
            return Ok(false);
        };
        let Ok(expires_at) = expiry.parse::<i64>() else {
            return Ok(false);
        };
        if expires_at <= now || expires_at.saturating_sub(now) > CONSENT_TOKEN_TTL_SECS {
            return Ok(false);
        }
        let expected = self.hash_at_rest(&binding.material(expires_at))?;
        Ok(bool::from(expected.as_bytes().ct_eq(mac.as_bytes())))
    }

    pub async fn is_declared_client(&self, client_id: &ClientId) -> OauthResult<bool> {
        let declared = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM oauth_clients
                WHERE client_id = $1 AND is_active AND declared_source IS NOT NULL
            ) AS "declared!"
            "#,
            client_id.as_str()
        )
        .fetch_one(self.write_pool_ref())
        .await?;
        Ok(declared)
    }

    pub async fn consent_covers(
        &self,
        user_id: &UserId,
        client_id: &ClientId,
        scopes: &[String],
    ) -> OauthResult<bool> {
        if scopes.is_empty() {
            return Ok(false);
        }
        let covered = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM oauth_consents k
                JOIN oauth_clients c ON c.client_id = k.client_id
                WHERE k.user_id = $1 AND k.client_id = $2 AND k.scopes @> $3
                  AND c.is_active AND c.declared_source IS NOT NULL
            ) AS "covered!"
            "#,
            user_id.as_str(),
            client_id.as_str(),
            scopes
        )
        .fetch_one(self.write_pool_ref())
        .await?;
        Ok(covered)
    }

    pub async fn record_consent(
        &self,
        user_id: &UserId,
        client_id: &ClientId,
        scopes: &[String],
    ) -> OauthResult<bool> {
        if scopes.is_empty() {
            return Ok(false);
        }
        let stored = sqlx::query!(
            r#"
            INSERT INTO oauth_consents (user_id, client_id, scopes)
            SELECT $1, c.client_id, $3::text[]
            FROM oauth_clients c
            WHERE c.client_id = $2 AND c.is_active AND c.declared_source IS NOT NULL
            ON CONFLICT (user_id, client_id) DO UPDATE
            SET scopes = ARRAY(
                    SELECT DISTINCT granted
                    FROM unnest(oauth_consents.scopes || EXCLUDED.scopes) AS granted
                    ORDER BY granted
                ),
                updated_at = CURRENT_TIMESTAMP
            "#,
            user_id.as_str(),
            client_id.as_str(),
            scopes
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(stored.rows_affected() > 0)
    }
}
