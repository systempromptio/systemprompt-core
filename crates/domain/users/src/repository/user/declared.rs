//! Rows the identity config plane manages: users whose `declared_source`
//! names the declaration file they come from.
//!
//! The plane reads and writes only these rows on the apply transaction. An
//! upsert keyed on the normalised email takes over an existing row of the same
//! account kind and stamps it declared; a row of the other kind is refused,
//! never converted. Releasing a row (its declaration was removed) clears the
//! stamp and deactivates it, so access granted from Git is withdrawn from Git
//! and the row's history stays.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use sqlx::PgConnection;
use systemprompt_identifiers::UserId;

use crate::error::{Result, UserError};
use crate::models::{UserStatus, normalise_email};
use crate::repository::UserRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredUserRow {
    pub email: String,
    pub name: String,
    pub account_kind: String,
    pub roles: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DeclaredUserWrite<'a> {
    pub email: &'a str,
    pub name: &'a str,
    pub account_kind: &'a str,
    pub roles: &'a [String],
    pub status: &'a str,
    pub source: &'a str,
}

impl UserRepository {
    pub async fn list_declared_on(conn: &mut PgConnection) -> Result<Vec<DeclaredUserRow>> {
        let rows = sqlx::query_as!(
            DeclaredUserRow,
            r#"
            SELECT email, name, account_kind, roles, status
            FROM users
            WHERE declared_source IS NOT NULL
            ORDER BY email
            "#,
        )
        .fetch_all(conn)
        .await?;
        Ok(rows)
    }

    pub async fn upsert_declared_on(
        conn: &mut PgConnection,
        user: &DeclaredUserWrite<'_>,
    ) -> Result<UserId> {
        let id = UserId::generate();
        let email = normalise_email(user.email);
        let verified = user.account_kind == crate::models::AccountKind::Service.as_str();
        let written = sqlx::query_scalar!(
            r#"
            INSERT INTO users (
                id, name, email, display_name, status, email_verified, roles, is_bot,
                account_kind, declared_source, created_at, updated_at
            )
            VALUES ($1, $2, $3, $2, $4, $5, $6, false, $7, $8, $9, $9)
            ON CONFLICT (email) DO UPDATE
            SET name = EXCLUDED.name,
                status = EXCLUDED.status,
                roles = EXCLUDED.roles,
                declared_source = EXCLUDED.declared_source,
                updated_at = EXCLUDED.updated_at
            WHERE users.account_kind = EXCLUDED.account_kind
            RETURNING id
            "#,
            id.as_str(),
            user.name,
            email,
            user.status,
            verified,
            user.roles,
            user.account_kind,
            user.source,
            Utc::now()
        )
        .fetch_optional(conn)
        .await?;
        written
            .map(UserId::new)
            .ok_or(UserError::AccountKindMismatch(email))
    }

    pub async fn release_declared_on(conn: &mut PgConnection, email: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET declared_source = NULL, status = $2, updated_at = $3
            WHERE email = $1 AND declared_source IS NOT NULL
            "#,
            normalise_email(email),
            UserStatus::Inactive.as_str(),
            Utc::now()
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn find_id_by_email_on(
        conn: &mut PgConnection,
        email: &str,
    ) -> Result<Option<UserId>> {
        let id = sqlx::query_scalar!(
            "SELECT id FROM users WHERE email = $1",
            normalise_email(email)
        )
        .fetch_optional(conn)
        .await?;
        Ok(id.map(UserId::new))
    }

    pub async fn emails_by_ids_on(
        conn: &mut PgConnection,
        ids: &[UserId],
    ) -> Result<Vec<(UserId, String)>> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            "SELECT id, email FROM users WHERE id = ANY($1::text[])",
            &ids
        )
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (UserId::new(row.id), row.email))
            .collect())
    }
}
