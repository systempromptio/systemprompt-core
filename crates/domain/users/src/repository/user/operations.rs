//! User row creation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::models::{User, UserRole, UserStatus, normalise_email};
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
        let email = normalise_email(email);

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
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(row)
    }

    pub async fn create_if_absent(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<Option<User>> {
        let now = Utc::now();
        let id = UserId::new(uuid::Uuid::new_v4().to_string());
        let display_name_val = display_name.or(full_name);
        let status = UserStatus::Active.as_str();
        let role = UserRole::User.as_str();
        let email = normalise_email(email);

        let row = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                id, name, email, full_name, display_name,
                status, email_verified, roles, is_bot,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, false, ARRAY[$7]::TEXT[], false, $8, $8)
            ON CONFLICT DO NOTHING
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
        .fetch_optional(&*self.write_pool)
        .await?;

        Ok(row)
    }

    pub async fn create_anonymous(&self, fingerprint: &str) -> Result<User> {
        let email = normalise_email(&format!("{}@anonymous.local", fingerprint));

        if let Some(existing) = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&*self.pool)
        .await?
        {
            return Ok(existing);
        }

        let user_id = uuid::Uuid::new_v4();
        let id = UserId::new(user_id.to_string());
        let name = format!("anonymous_{}", &user_id.to_string()[..8]);
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
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(row)
    }
}
