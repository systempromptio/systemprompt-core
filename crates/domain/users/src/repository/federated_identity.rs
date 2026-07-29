//! Repository for `federated_identities` — the `{issuer, external_sub} ->
//! users.id` mapping used by RFC 8693 token-exchange first-touch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use sqlx::Acquire;
use systemprompt_identifiers::UserId;
use systemprompt_traits::FederatedIdentityClaims;

use crate::error::Result;
use crate::models::{User, UserRole, UserStatus, normalise_email};
use crate::repository::UserRepository;

impl UserRepository {
    pub async fn find_federated(&self, issuer: &str, external_sub: &str) -> Result<Option<UserId>> {
        let row = sqlx::query!(
            "SELECT user_id FROM federated_identities WHERE issuer = $1 AND external_sub = $2",
            issuer,
            external_sub
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| UserId::new(r.user_id)))
    }

    pub async fn find_or_create_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
    ) -> Result<User> {
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;

        if let Some(existing) = sqlx::query!(
            "UPDATE federated_identities SET last_seen_at = CURRENT_TIMESTAMP WHERE issuer = $1 \
             AND external_sub = $2 RETURNING user_id",
            issuer,
            external_sub
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            let user = sqlx::query_as!(
                User,
                r#"
                SELECT id, name, email, full_name, display_name, status,
                       email_verified, roles, avatar_url, is_bot, is_scanner,
                       created_at, updated_at
                FROM users WHERE id = $1
                "#,
                existing.user_id
            )
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(user);
        }

        if let Some(existing) =
            link_by_verified_email(&mut tx, issuer, external_sub, claims).await?
        {
            tx.commit().await?;
            return Ok(existing);
        }

        let fields = NewFederatedUser::derive(issuer, external_sub, claims);

        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                id, name, email, full_name, display_name,
                status, email_verified, roles, is_bot,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, false, $7::TEXT[], false, $8, $8)
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            fields.id.as_str(),
            fields.name,
            fields.email,
            fields.display_name.as_deref(),
            fields.display_name.as_deref(),
            fields.status,
            &fields.roles,
            fields.now,
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
            issuer,
            external_sub,
            user.id.as_str()
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(user)
    }
}

// Why: a verified upstream email attaches this sign-in to the existing
// account instead of minting a duplicate user — one human, one row.
// Unverified emails never link (account-claim defence in `derive`).
async fn link_by_verified_email(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    issuer: &str,
    external_sub: &str,
    claims: &FederatedIdentityClaims,
) -> Result<Option<User>> {
    if !claims.email_verified {
        return Ok(None);
    }
    let Some(addr) = claims.email.as_deref() else {
        return Ok(None);
    };
    let email = normalise_email(addr);
    let deleted_status = UserStatus::Deleted.as_str();
    let Some(existing) = sqlx::query_as!(
        User,
        r#"
        SELECT id, name, email, full_name, display_name, status,
               email_verified, roles, avatar_url, is_bot, is_scanner,
               created_at, updated_at
        FROM users WHERE email = $1 AND status != $2
        "#,
        email,
        deleted_status
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };

    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
        issuer,
        external_sub,
        existing.id.as_str()
    )
    .execute(&mut **tx)
    .await?;
    Ok(Some(existing))
}

struct NewFederatedUser {
    id: UserId,
    name: String,
    email: String,
    display_name: Option<String>,
    status: &'static str,
    roles: Vec<String>,
    now: chrono::DateTime<Utc>,
}

impl NewFederatedUser {
    fn derive(issuer: &str, external_sub: &str, claims: &FederatedIdentityClaims) -> Self {
        let name = claims
            .preferred_username
            .clone()
            .or_else(|| claims.name.clone())
            .unwrap_or_else(|| format!("fed_{}_{}", short_hash(issuer), short_hash(external_sub)));
        let synthetic_email = || {
            format!(
                "{}@{}.federated.local",
                short_hash(external_sub),
                short_host(issuer)
            )
        };
        let email = match (claims.email.as_deref(), claims.email_verified) {
            (Some(addr), true) => normalise_email(addr),
            (Some(addr), false) => {
                tracing::warn!(
                    issuer,
                    external_sub,
                    upstream_email = addr,
                    "upstream IdP did not assert email_verified; using synthetic local email to \
                     prevent account-claim attacks"
                );
                synthetic_email()
            },
            (None, _) => synthetic_email(),
        };

        Self {
            id: UserId::new(uuid::Uuid::new_v4().to_string()),
            name,
            email,
            display_name: claims.name.clone(),
            status: UserStatus::Active.as_str(),
            roles: normalised_roles(&claims.roles),
            now: Utc::now(),
        }
    }
}

fn normalised_roles(claim_roles: &[String]) -> Vec<String> {
    if claim_roles.is_empty() {
        vec![UserRole::User.as_str().to_owned()]
    } else {
        claim_roles.to_vec()
    }
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(s.as_bytes());
    hex::encode(&digest[..6])
}

fn short_host(issuer: &str) -> String {
    issuer
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("issuer")
        .replace(['.', ':'], "-")
}
