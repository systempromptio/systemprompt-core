use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;

use crate::error::{Result, UserError};
use crate::models::{User, UserRole, UserStatus};
use crate::repository::UserRepository;

#[derive(Debug)]
pub struct UpdateUserParams<'a> {
    pub email: &'a str,
    pub full_name: Option<&'a str>,
    pub display_name: Option<&'a str>,
    pub status: UserStatus,
}

impl UserRepository {
    pub async fn create(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<User> {
        let now = Utc::now();
        let id = UserId::new(uuid::Uuid::new_v4().to_string());
        let display_name_val = display_name.or(full_name);
        let status = UserStatus::Active.as_str();
        let role = UserRole::User.as_str();

        let row = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                id, name, email, full_name, display_name,
                status, email_verified, roles, is_bot,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, false, ARRAY[$7]::TEXT[], false, $8, $8)
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            id.as_str(),
            name,
            email,
            full_name,
            display_name_val,
            status,
            role,
            now
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn create_anonymous(&self, fingerprint: &str) -> Result<User> {
        let user_id = uuid::Uuid::new_v4();
        let id = UserId::new(user_id.to_string());
        let name = format!("anonymous_{}", &user_id.to_string()[..8]);
        let email = format!("{}@anonymous.local", fingerprint);
        let now = Utc::now();
        let status = UserStatus::Active.as_str();
        let role = UserRole::Anonymous.as_str();

        let row = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                id, name, email, status, email_verified, roles,
                is_bot, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, false, ARRAY[$5]::TEXT[], false, $6, $6)
            ON CONFLICT (email) DO UPDATE SET updated_at = $6
            RETURNING id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            "#,
            id.as_str(),
            name,
            email,
            status,
            role,
            now
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row)
    }

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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
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
        .fetch_optional(&*self.pool)
        .await?
        .ok_or_else(|| UserError::NotFound(id.clone()))?;

        Ok(row)
    }

    pub async fn delete(&self, id: &UserId) -> Result<()> {
        let result = sqlx::query!(
            r#"DELETE FROM users WHERE id = $1"#,
            id.as_str()
        )
        .execute(&*self.pool)
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
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Merge source user into target user:
    /// - Transfer all sessions from source to target
    /// - Transfer all tasks from source to target
    /// - Delete source user
    /// Returns the number of sessions transferred
    pub async fn merge_users(&self, source_id: &UserId, target_id: &UserId) -> Result<MergeResult> {
        // Transfer sessions
        let sessions_result = sqlx::query!(
            r#"
            UPDATE user_sessions
            SET user_id = $1
            WHERE user_id = $2
            "#,
            target_id.as_str(),
            source_id.as_str()
        )
        .execute(&*self.pool)
        .await?;

        // Transfer tasks (if table exists)
        let tasks_result = sqlx::query!(
            r#"
            UPDATE agent_tasks
            SET user_id = $1
            WHERE user_id = $2
            "#,
            target_id.as_str(),
            source_id.as_str()
        )
        .execute(&*self.pool)
        .await
        .unwrap_or_default();

        // Delete source user
        sqlx::query!(
            r#"DELETE FROM users WHERE id = $1"#,
            source_id.as_str()
        )
        .execute(&*self.pool)
        .await?;

        Ok(MergeResult {
            sessions_transferred: sessions_result.rows_affected(),
            tasks_transferred: tasks_result.rows_affected(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct MergeResult {
    pub sessions_transferred: u64,
    pub tasks_transferred: u64,
}
