use super::OAuthRepository;
use anyhow::Result;
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
    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<OAuthUser>> {
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
        user_id: &str,
    ) -> Result<systemprompt_models::auth::AuthenticatedUser> {
        let row = sqlx::query!(
            "SELECT id, name, email, roles FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(self.pool_ref())
        .await?
        .ok_or_else(|| anyhow::anyhow!("User not found: {user_id}"))?;

        let permissions: Vec<Permission> = row
            .roles
            .iter()
            .filter_map(|s| Permission::from_str(s).ok())
            .collect();

        if permissions.is_empty() {
            return Err(anyhow::anyhow!(
                "User has no valid permissions after parsing"
            ));
        }

        let user_uuid = Uuid::parse_str(&row.id)
            .map_err(|_| anyhow::anyhow!("Invalid user UUID: {}", row.id))?;

        Ok(
            systemprompt_models::auth::AuthenticatedUser::new_with_roles(
                user_uuid,
                row.name,
                Some(row.email),
                permissions,
                row.roles,
            ),
        )
    }
}
