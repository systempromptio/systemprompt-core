//! OAuth user record helpers.

use super::OAuthRepository;
use crate::error::{OauthError, OauthResult};
use std::str::FromStr;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::Permission;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OAuthUser {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
}

impl OAuthRepository {
    pub async fn find_user_by_email(&self, email: &str) -> OauthResult<Option<OAuthUser>> {
        let row = sqlx::query!(
            "SELECT id, name, email, roles FROM users WHERE email = $1",
            email
        )
        .fetch_optional(self.pool_ref())
        .await?;

        Ok(row.map(|r| OAuthUser {
            id: UserId::new(r.id),
            name: r.name,
            email: r.email,
            roles: r.roles,
        }))
    }

    pub async fn get_authenticated_user(
        &self,
        user_id: &UserId,
    ) -> OauthResult<systemprompt_models::auth::AuthenticatedUser> {
        let user_id_str = user_id.as_str();
        let row = sqlx::query!(
            "SELECT id, name, email, roles FROM users WHERE id = $1",
            user_id_str
        )
        .fetch_optional(self.pool_ref())
        .await?
        .ok_or_else(|| OauthError::UserNotFound(user_id.to_string()))?;

        let permissions: Vec<Permission> = row
            .roles
            .iter()
            .filter_map(|s| {
                Permission::from_str(s)
                    .map_err(|e| {
                        tracing::warn!(
                            user_id = %row.id,
                            role = %s,
                            error = %e,
                            "Invalid role in database"
                        );
                        e
                    })
                    .ok()
            })
            .collect();

        if permissions.is_empty() {
            return Err(OauthError::Validation(
                "User has no valid permissions after parsing".to_string(),
            ));
        }

        let user_uuid = Uuid::parse_str(&row.id)
            .map_err(|_| OauthError::Validation(format!("Invalid user UUID: {}", row.id)))?;

        Ok(
            systemprompt_models::auth::AuthenticatedUser::new_with_roles(
                user_uuid,
                row.name,
                row.email,
                permissions,
                row.roles,
            ),
        )
    }
}
