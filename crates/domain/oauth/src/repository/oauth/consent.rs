//! Remembered authorization consent, the binding of a consent decision, and
//! the re-authentication marker.
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
//! A re-authentication marker is minted the same way, under its own domain,
//! over the [`ReauthBinding`] and the time the endpoint sent the person to
//! sign in. A marker that is malformed, older than [`REAUTH_MARKER_TTL_SECS`],
//! dated in the future or minted for another request verifies as no marker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use subtle::ConstantTimeEq;
use systemprompt_identifiers::{ClientId, SessionId, UserId};

use super::OAuthRepository;
use crate::error::OauthResult;

pub const CONSENT_TOKEN_TTL_SECS: i64 = 600;
pub const REAUTH_MARKER_TTL_SECS: i64 = 600;

const CONSENT_TOKEN_DOMAIN: &str = "authorize-consent-v2";
const REAUTH_MARKER_DOMAIN: &str = "authorize-reauth-v1";

/// The authorization request a consent decision belongs to.
///
/// It names who is signed in, through which browser session, and the client,
/// redirect, scope, `state`, PKCE challenge and method, response type and
/// resource the code would be minted for.
#[derive(Debug, Clone, Copy)]
pub struct ConsentBinding<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub client_id: &'a ClientId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub state: &'a str,
    pub code_challenge: &'a str,
    pub code_challenge_method: &'a str,
    pub response_type: &'a str,
    pub resource: Option<&'a str>,
}

/// The authorization request a re-authentication marker belongs to, as its
/// query states it.
#[derive(Debug, Clone, Copy)]
pub struct ReauthBinding<'a> {
    pub client_id: &'a ClientId,
    pub response_type: &'a str,
    pub redirect_uri: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub state: Option<&'a str>,
    pub code_challenge: Option<&'a str>,
    pub code_challenge_method: Option<&'a str>,
    pub resource: Option<&'a str>,
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("+{value}"))
}

fn length_prefixed(fields: &[&str]) -> String {
    let mut material = String::new();
    for field in fields {
        material.push_str(&field.len().to_string());
        material.push(':');
        material.push_str(field);
    }
    material
}

fn split_stamped(presented: &str) -> Option<(i64, &str)> {
    let (stamp, mac) = presented.split_once('.')?;
    Some((stamp.parse::<i64>().ok()?, mac))
}

impl ConsentBinding<'_> {
    fn material(&self, expires_at: i64) -> String {
        let resource = optional(self.resource);
        let expiry = expires_at.to_string();
        length_prefixed(&[
            CONSENT_TOKEN_DOMAIN,
            self.user_id.as_str(),
            self.session_id.as_str(),
            self.client_id.as_str(),
            self.redirect_uri,
            self.scope,
            self.state,
            self.code_challenge,
            self.code_challenge_method,
            self.response_type,
            resource.as_str(),
            expiry.as_str(),
        ])
    }
}

impl ReauthBinding<'_> {
    fn material(&self, issued_at: i64) -> String {
        let optionals = [
            self.redirect_uri,
            self.scope,
            self.state,
            self.code_challenge,
            self.code_challenge_method,
            self.resource,
        ]
        .map(optional);
        let issued = issued_at.to_string();
        let mut fields = vec![
            REAUTH_MARKER_DOMAIN,
            self.client_id.as_str(),
            self.response_type,
        ];
        fields.extend(optionals.iter().map(String::as_str));
        fields.push(issued.as_str());
        length_prefixed(&fields)
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
        let Some((expires_at, mac)) = split_stamped(presented) else {
            return Ok(false);
        };
        if expires_at <= now || expires_at.saturating_sub(now) > CONSENT_TOKEN_TTL_SECS {
            return Ok(false);
        }
        let expected = self.hash_at_rest(&binding.material(expires_at))?;
        Ok(bool::from(expected.as_bytes().ct_eq(mac.as_bytes())))
    }

    pub fn reauth_marker(
        &self,
        binding: &ReauthBinding<'_>,
        issued_at: i64,
    ) -> OauthResult<String> {
        let mac = self.hash_at_rest(&binding.material(issued_at))?;
        Ok(format!("{issued_at}.{mac}"))
    }

    pub fn verified_reauth_since(
        &self,
        binding: &ReauthBinding<'_>,
        presented: Option<&str>,
        now: i64,
    ) -> OauthResult<Option<i64>> {
        let Some((issued_at, mac)) = presented.and_then(split_stamped) else {
            return Ok(None);
        };
        if issued_at > now || now.saturating_sub(issued_at) > REAUTH_MARKER_TTL_SECS {
            return Ok(None);
        }
        let expected = self.hash_at_rest(&binding.material(issued_at))?;
        Ok(bool::from(expected.as_bytes().ct_eq(mac.as_bytes())).then_some(issued_at))
    }

    pub async fn is_declared_client(&self, client_id: &ClientId) -> OauthResult<bool> {
        let declared = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM oauth_clients
                WHERE client_id = $1 AND is_active AND declared_source IS NOT NULL
            ) AS "declared!"
            "#,
            client_id as _
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
            user_id as _,
            client_id as _,
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
            user_id as _,
            client_id as _,
            scopes
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(stored.rows_affected() > 0)
    }
}
