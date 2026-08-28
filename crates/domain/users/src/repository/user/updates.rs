//! User record field updates, role assignment, and anonymous-account cleanup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;

use super::operations::UpdateUserParams;
use crate::error::{Result, UserError};
use crate::models::{User, UserRole, UserStatus};
use crate::repository::UserRepository;

impl UserRepository {
    pub async fn update_email(&self, id: &UserId, email: &str) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET email = $1, email_verified = false, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            email,
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn update_full_name(&self, id: &UserId, full_name: &str) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET full_name = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            full_name,
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn update_status(&self, id: &UserId, status: UserStatus) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET status = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            status.as_str(),
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn update_email_verified(&self, id: &UserId, verified: bool) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET email_verified = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            verified,
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn update_display_name(&self, id: &UserId, display_name: &str) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET display_name = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            display_name,
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn update_all_fields(
        &self,
        id: &UserId,
        params: UpdateUserParams<'_>,
    ) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET email = $1, full_name = $2, display_name = $3, status = $4, updated_at = $5
            WHERE id = $6
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            params.email,
            params.full_name,
            params.display_name,
            params.status.as_str(),
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn assign_roles(&self, id: &UserId, roles: &[String]) -> Result<User> {
        let row = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET roles = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            roles,
            Utc::now(),
            id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn delete(&self, id: &UserId) -> Result<()> {
        let result = sqlx::query!(r#"DELETE FROM users WHERE id = $1"#, id.as_str())
            .execute(&*self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(UserError::NotFound(id.clone()));
        }

        Ok(())
    }

    pub async fn cleanup_old_anonymous(&self, days: i32) -> Result<u64> {
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        let anonymous_role = UserRole::Anonymous.as_str();
        let result = sqlx::query!(
            r#"
            DELETE FROM users u
            WHERE $1 = ANY(u.roles)
              AND u.created_at < $2
              AND NOT EXISTS (
                  SELECT 1
                  FROM user_sessions s
                  WHERE s.user_id = u.id
                    AND s.ended_at IS NULL
              )
            "#,
            anonymous_role,
            cutoff
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn count_old_anonymous(&self, days: i32) -> Result<i64> {
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        let anonymous_role = UserRole::Anonymous.as_str();
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM users u
            WHERE $1 = ANY(u.roles)
              AND u.created_at < $2
              AND NOT EXISTS (
                  SELECT 1
                  FROM user_sessions s
                  WHERE s.user_id = u.id
                    AND s.ended_at IS NULL
              )
            "#,
            anonymous_role,
            cutoff
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(count)
    }
}
