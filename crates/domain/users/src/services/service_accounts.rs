//! [`UsersServiceAccounts`]: the users member's `ServiceAccounts` directory
//! over the user repository's service-account upsert.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_traits::{
    ServiceAccount, ServiceAccountError, ServiceAccountRequest, ServiceAccounts,
};

use crate::repository::{NewServiceAccount, UserRepository};

#[derive(Debug, Clone)]
pub struct UsersServiceAccounts {
    users: UserRepository,
}

impl UsersServiceAccounts {
    #[must_use]
    pub const fn new(users: UserRepository) -> Self {
        Self { users }
    }
}

#[async_trait]
impl ServiceAccounts for UsersServiceAccounts {
    async fn ensure(
        &self,
        request: ServiceAccountRequest,
    ) -> Result<ServiceAccount, ServiceAccountError> {
        let ensured = self
            .users
            .ensure_service_account(NewServiceAccount {
                name: request.name,
                email: request.email,
                created_by: request.created_by,
            })
            .await
            .map_err(|e| ServiceAccountError(Box::new(e)))?;
        Ok(ServiceAccount {
            service: ensured.user.is_service_account(),
            id: ensured.user.id,
            roles: ensured.user.roles,
        })
    }
}
