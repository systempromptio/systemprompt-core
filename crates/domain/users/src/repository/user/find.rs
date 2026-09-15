//! User lookup queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::models::{User, UserRole, UserStatus};
use crate::repository::UserRepository;

impl UserRepository {
    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE id = $1 AND status != $2
            "#,
            id.as_str(),
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let email = crate::models::normalise_email(email);
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE email = $1 AND status != $2
            "#,
            email,
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE name = $1 AND status != $2
            ORDER BY created_at ASC
            LIMIT 1
            "#,
            name,
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_by_role(&self, role: UserRole) -> Result<Vec<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE $1 = ANY(roles) AND status != $2
            ORDER BY created_at DESC
            "#,
            role.as_str(),
            deleted_status
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn find_first_user(&self) -> Result<Option<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE status != $1
            ORDER BY created_at ASC
            LIMIT 1
            "#,
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_first_admin(&self) -> Result<Option<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let admin_role = UserRole::Admin.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE $1 = ANY(roles) AND status != $2
            ORDER BY created_at ASC
            LIMIT 1
            "#,
            admin_role,
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_authenticated_user(&self, user_id: &UserId) -> Result<Option<User>> {
        let active_status = UserStatus::Active.as_str();
        let row = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE id = $1 AND status = $2
            "#,
            user_id.as_str(),
            active_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }
}

impl UserRepository {
    pub async fn missing_ids(&self, candidates: &[UserId]) -> Result<Vec<UserId>> {
        let ids: Vec<String> = candidates.iter().map(ToString::to_string).collect();
        let rows = sqlx::query_scalar!(
            r#"
            SELECT candidate AS "candidate!"
            FROM UNNEST($1::text[]) AS candidate
            WHERE NOT EXISTS (SELECT 1 FROM users WHERE users.id = candidate)
            "#,
            &ids[..]
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(UserId::new).collect())
    }
}
