use crate::error::Result;
use crate::models::User;
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
        if identifier.contains('@') {
            self.user_service.find_by_email(identifier).await
        } else {
            self.user_service.find_by_name(identifier).await
        }
    }

    pub async fn promote_to_admin(&self, user_identifier: &str) -> Result<PromoteResult> {
        let user = self.find_user(user_identifier).await?;

        match user {
            Some(u) => {
                let current_roles = u.roles.clone();

                if current_roles.contains(&"admin".to_string()) {
                    return Ok(PromoteResult::AlreadyAdmin(u));
                }

                let mut new_roles = current_roles;
                if !new_roles.contains(&"admin".to_string()) {
                    new_roles.push("admin".to_string());
                }
                if !new_roles.contains(&"user".to_string()) {
                    new_roles.push("user".to_string());
                }

                let updated = self.user_service.assign_roles(&u.id, &new_roles).await?;
                Ok(PromoteResult::Promoted(updated, new_roles))
            },
            None => Ok(PromoteResult::UserNotFound),
        }
    }

    pub async fn demote_from_admin(&self, user_identifier: &str) -> Result<DemoteResult> {
        let user = self.find_user(user_identifier).await?;

        match user {
            Some(u) => {
                let current_roles = u.roles.clone();

                if !current_roles.contains(&"admin".to_string()) {
                    return Ok(DemoteResult::NotAdmin(u));
                }

                let new_roles: Vec<String> =
                    current_roles.into_iter().filter(|r| r != "admin").collect();

                let mut final_roles = new_roles;
                if !final_roles.contains(&"user".to_string()) {
                    final_roles.push("user".to_string());
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
