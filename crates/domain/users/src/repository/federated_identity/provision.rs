//! First-touch provisioning of a local account for a federated identity the
//! issuer may own: its name, email and roles are derived from the claims.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::UserId;
use systemprompt_traits::FederatedIdentityClaims;

use crate::error::{Result, UserError};
use crate::models::{User, UserRole, UserRow, UserStatus, normalise_email};

pub(super) async fn insert_federated_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    issuer: &str,
    external_sub: &str,
    claims: &FederatedIdentityClaims,
) -> Result<User> {
    let fields = NewFederatedUser::derive(issuer, external_sub, claims);

    let user = sqlx::query_as!(
        UserRow,
        r#"
        INSERT INTO users (
            id, name, email, full_name, display_name,
            status, email_verified, roles, is_bot,
            created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, false, $7::TEXT[], false, $8, $8)
        RETURNING id, name, email, full_name, display_name, status, email_verified,
                  roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
        "#,
        fields.id.as_str(),
        fields.name,
        fields.email,
        fields.display_name.as_deref(),
        fields.display_name.as_deref(),
        fields.status.as_str(),
        &fields.roles,
        fields.now,
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(UserError::from)
    .and_then(User::try_from)?;

    super::insert_link(tx, issuer, external_sub, &user.id, true).await?;
    Ok(user)
}

struct NewFederatedUser {
    id: UserId,
    name: String,
    email: String,
    display_name: Option<String>,
    status: UserStatus,
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
            id: UserId::generate(),
            name,
            email,
            display_name: claims.name.clone(),
            status: UserStatus::Active,
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
