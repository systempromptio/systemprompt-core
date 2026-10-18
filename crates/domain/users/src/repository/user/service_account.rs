//! Service-account rows: the non-human identity a workflow acts as.
//!
//! A service account is created active with the plain `user` role and no
//! interactive credential. Its credentials are API keys and in-process JWTs;
//! every interactive login path refuses it by `account_kind`.
//!
//! Creation is idempotent on the normalised email: ensuring an account that
//! already exists returns it unchanged, and an email held by a human account
//! is refused rather than converted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::UserId;

use crate::error::{Result, UserError};
use crate::models::{AccountKind, User, UserRole, UserRow, UserStatus, normalise_email};
use crate::repository::UserRepository;

#[derive(Debug, Clone)]
pub struct NewServiceAccount {
    pub name: String,
    pub email: String,
    pub created_by: UserId,
}

#[derive(Debug, Clone)]
pub struct EnsuredServiceAccount {
    pub user: User,
    pub created: bool,
}

impl UserRepository {
    pub async fn ensure_service_account(
        &self,
        account: NewServiceAccount,
    ) -> Result<EnsuredServiceAccount> {
        let id = UserId::generate();
        let email = normalise_email(&account.email);
        let inserted = sqlx::query_as!(
            UserRow,
            r#"
            INSERT INTO users (
                id, name, email, display_name, status, email_verified, roles, is_bot,
                account_kind, created_at, updated_at
            )
            VALUES ($1, $2, $3, $2, $4, true, ARRAY[$5]::TEXT[], false, $6, $7, $7)
            ON CONFLICT (email) DO NOTHING
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
            "#,
            id.as_str(),
            account.name,
            email,
            UserStatus::Active.as_str(),
            UserRole::User.as_str(),
            AccountKind::Service.as_str(),
            Utc::now()
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        if let Some(row) = inserted {
            let user = User::try_from(row)?;
            tracing::info!(
                user_id = %user.id,
                created_by = %account.created_by,
                "service account created"
            );
            return Ok(EnsuredServiceAccount {
                user,
                created: true,
            });
        }
        let existing = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
            FROM users
            WHERE email = $1
            "#,
            email
        )
        .fetch_one(&*self.write_pool)
        .await
        .map_err(UserError::from)
        .and_then(User::try_from)?;
        if !existing.is_service_account() || existing.status == UserStatus::Deleted {
            return Err(UserError::EmailAlreadyExists(email));
        }
        Ok(EnsuredServiceAccount {
            user: existing,
            created: false,
        })
    }

    pub async fn list_service_accounts(&self) -> Result<Vec<User>> {
        let rows = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
            FROM users
            WHERE account_kind = $1 AND status != $2
            ORDER BY created_at
            "#,
            AccountKind::Service.as_str(),
            UserStatus::Deleted.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        rows.into_iter().map(User::try_from).collect()
    }
}
