use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_models::services::SystemAdmin;
use systemprompt_users::{UserRepository, UserService};

#[must_use]
pub fn fixture_user_id() -> UserId {
    UserId::new("test-user")
}

#[must_use]
pub fn unique_user_id(prefix: &str) -> UserId {
    UserId::new(format!("{prefix}-{}", uuid::Uuid::new_v4()))
}

#[must_use]
pub fn fixture_actor() -> Actor {
    Actor::user(fixture_user_id())
}

#[must_use]
pub fn fixture_system_admin(username: &str) -> SystemAdmin {
    SystemAdmin::new(unique_user_id("admin"), username.to_string())
}

pub fn merging_user_service(pool: &DbPool) -> anyhow::Result<UserService> {
    Ok(UserService::new(Arc::new(UserRepository::new(pool)))
        .with_owner_reassignments(systemprompt_runtime::owner_reassignments(pool)))
}
