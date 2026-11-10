//! Repository for `federated_identities` — the `{issuer, external_sub} ->
//! users.id` mapping every federated sign-in resolves through.
//!
//! [`UserRepository::resolve_federated`] is the only way an external identity
//! reaches a local account: SSO, token exchange and chat-platform senders all
//! call it. Under one transaction it returns an existing link, links an
//! eligible account by verified email when the issuer's [`LinkPolicy`] allows
//! it, refuses a verified email an existing account owns otherwise, or
//! provisions a fresh account. A link records whether the issuer provisioned
//! its account, which decides whether the issuer's roles may be written to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod provision;

use sqlx::Acquire;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{FederatedIdentityClaims, FederatedLink, FederatedRefusal, LinkPolicy};

use crate::error::{Result, UserError};
use crate::models::{User, UserRole, UserRow, UserStatus, normalise_email};
use crate::repository::UserRepository;

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

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

    pub async fn resolve_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
        policy: &LinkPolicy,
    ) -> Result<(User, FederatedLink)> {
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;

        lock_first_touch(&mut tx, &format!("federated:{issuer}\n{external_sub}")).await?;
        let verified_email = verified_email(claims);
        if let Some(email) = verified_email.as_deref() {
            lock_first_touch(&mut tx, &format!("email:{email}")).await?;
        }

        if let Some((user, provisioned)) =
            touch_existing_link(&mut tx, issuer, external_sub).await?
        {
            tx.commit().await?;
            return Ok((user, FederatedLink::Existing { provisioned }));
        }

        if let Some(email) = verified_email.as_deref()
            && let Some(owner) = find_email_owner(&mut tx, email).await?
        {
            if !policy.link_by_email {
                return Err(UserError::FederatedRefused(FederatedRefusal::EmailInUse));
            }
            if !is_linkable(&owner) {
                return Err(UserError::FederatedRefused(
                    FederatedRefusal::TargetIneligible,
                ));
            }
            insert_link(&mut tx, issuer, external_sub, &owner.id, false).await?;
            tx.commit().await?;
            return Ok((owner, FederatedLink::Linked));
        }

        if !policy.auto_provision {
            return Err(UserError::FederatedRefused(
                FederatedRefusal::NotProvisioned,
            ));
        }
        let user = provision::insert_federated_user(&mut tx, issuer, external_sub, claims).await?;
        tx.commit().await?;
        Ok((user, FederatedLink::Provisioned))
    }
}

fn verified_email(claims: &FederatedIdentityClaims) -> Option<String> {
    claims
        .email
        .as_deref()
        .filter(|_| claims.email_verified)
        .map(normalise_email)
}

fn is_linkable(user: &User) -> bool {
    user.status == UserStatus::Active
        && !user.is_service_account()
        && !user.is_bot
        && user
            .roles
            .iter()
            .all(|role| role == UserRole::User.as_str())
}

async fn lock_first_touch(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))", key)
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_link(
    tx: &mut Tx<'_>,
    issuer: &str,
    external_sub: &str,
    user_id: &UserId,
    provisioned: bool,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id, provisioned) VALUES \
         ($1, $2, $3, $4)",
        issuer,
        external_sub,
        user_id.as_str(),
        provisioned
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn touch_existing_link(
    tx: &mut Tx<'_>,
    issuer: &str,
    external_sub: &str,
) -> Result<Option<(User, bool)>> {
    let Some(existing) = sqlx::query!(
        "UPDATE federated_identities SET last_seen_at = CURRENT_TIMESTAMP WHERE issuer = $1 \
         AND external_sub = $2 RETURNING user_id, provisioned",
        issuer,
        external_sub
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let user = sqlx::query_as!(
        UserRow,
        r#"
        SELECT id, name, email, full_name, display_name, status,
               email_verified, roles, avatar_url, is_bot, is_scanner,
               created_at, updated_at, account_kind
        FROM users WHERE id = $1
        "#,
        existing.user_id
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(UserError::from)
    .and_then(User::try_from)?;
    Ok(Some((user, existing.provisioned)))
}

async fn find_email_owner(tx: &mut Tx<'_>, email: &str) -> Result<Option<User>> {
    let deleted_status = UserStatus::Deleted.as_str();
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT id, name, email, full_name, display_name, status,
               email_verified, roles, avatar_url, is_bot, is_scanner,
               created_at, updated_at, account_kind
        FROM users WHERE email = $1 AND status != $2
        "#,
        email,
        deleted_status
    )
    .fetch_optional(&mut **tx)
    .await?
    .map(User::try_from)
    .transpose()
}
