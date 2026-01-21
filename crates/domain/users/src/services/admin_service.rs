use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::models::{User, UserRole};
use crate::UserService;

#[derive(Debug)]
pub struct UserAdminService {
    user_service: UserService,
}

impl UserAdminService {
    pub const fn new(user_service: UserService) -> Self {
        Self { user_service }
    }

    pub async fn find_user(&self, identifier: &str) -> Result<Option<User>> {
        if uuid::Uuid::parse_str(identifier).is_ok() {
            let user_id = UserId::new(identifier);
            if let Some(user) = self.user_service.find_by_id(&user_id).await? {
                return Ok(Some(user));
            }
        }

        if identifier.contains('@') {
            return self.user_service.find_by_email(identifier).await;
        }

        self.user_service.find_by_name(identifier).await
    }

    pub async fn promote_to_admin(&self, user_identifier: &str) -> Result<PromoteResult> {
        let user = self.find_user(user_identifier).await?;
        let admin_role = UserRole::Admin.as_str().to_string();
        let user_role = UserRole::User.as_str().to_string();

        match user {
            Some(u) => {
                let current_roles = u.roles.clone();

                if current_roles.contains(&admin_role) {
                    return Ok(PromoteResult::AlreadyAdmin(u));
                }

                let mut new_roles = current_roles;
                if !new_roles.contains(&admin_role) {
                    new_roles.push(admin_role);
                }
                if !new_roles.contains(&user_role) {
                    new_roles.push(user_role);
                }

                let updated = self.user_service.assign_roles(&u.id, &new_roles).await?;
                Ok(PromoteResult::Promoted(updated, new_roles))
            },
            None => Ok(PromoteResult::UserNotFound),
        }
    }

    pub async fn demote_from_admin(&self, user_identifier: &str) -> Result<DemoteResult> {
        let user = self.find_user(user_identifier).await?;
        let admin_role = UserRole::Admin.as_str();
        let user_role = UserRole::User.as_str().to_string();

        match user {
            Some(u) => {
                let current_roles = u.roles.clone();

                if !current_roles.contains(&admin_role.to_string()) {
                    return Ok(DemoteResult::NotAdmin(u));
                }

                let new_roles: Vec<String> = current_roles
                    .into_iter()
                    .filter(|r| r != admin_role)
                    .collect();

                let mut final_roles = new_roles;
                if !final_roles.contains(&user_role) {
                    final_roles.push(user_role);
                }

                let updated = self.user_service.assign_roles(&u.id, &final_roles).await?;
                Ok(DemoteResult::Demoted(updated, final_roles))
            },
            None => Ok(DemoteResult::UserNotFound),
        }
    }
}

#[derive(Debug)]
pub enum PromoteResult {
    Promoted(User, Vec<String>),
    AlreadyAdmin(User),
    UserNotFound,
}

#[derive(Debug)]
pub enum DemoteResult {
    Demoted(User, Vec<String>),
    NotAdmin(User),
    UserNotFound,
}
