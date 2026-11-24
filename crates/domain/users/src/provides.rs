//! The users member's composed provides: the sealed gateway session mint
//! conferred on `gateway-http`, the sealed sign-in session view conferred on
//! `identity-http`, the open request authenticator, the open per-user
//! rate-limit windows the API's global limiter charges, the pre-kernel
//! admin-bootstrap constructor the CLI host calls with an open database
//! handle, the sealed retention answers the kernel's `database_cleanup` job
//! reads (`DynUserRetention`), and the sealed user directory
//! (`DynUserProvider`: find, create and promote users, with the admitted owner
//! reassignments applied on merge), which only kernel code and core provide
//! fns read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedOwnerReassignments;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{DynRequestAuthenticator, DynTokenRevocations};
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_security::{
    JwtRequestAuthenticator, JwtRequestAuthenticatorParts, UnavailableRevocations,
};
use systemprompt_traits::{
    AdminAccount, AdminBootstrap, AdminBootstrapError, AdminGrant, AdminPromotion, DatabaseHandle,
    DynAdminBootstrap, DynGatewaySessionMint, DynRateLimitBuckets, DynSignInSessions,
    DynUserProvider, DynUserRetention,
};

use crate::repository::{SessionRepository, UserRateLimitBucketRepository, UserRepository};
use crate::services::UsersRetention;
use crate::{
    ApiKeyLifetime, PromoteResult, User, UserAdminService, UserRole, UserService,
    UsersGatewaySessionMint,
};

pub(crate) fn user_retention(r: &Resolved<'_>) -> Result<DynUserRetention, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(UsersRetention::new(Arc::new(
        UserRepository::new(&db),
    ))))
}

pub(crate) fn user_provider(r: &Resolved<'_>) -> Result<DynUserProvider, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(
        UserService::new(Arc::new(UserRepository::new(&db)))
            .with_owner_reassignments(r.get::<Arc<AdmittedOwnerReassignments>>()?),
    ))
}

pub(crate) fn gateway_session_mint(
    r: &Resolved<'_>,
) -> Result<DynGatewaySessionMint, ProvideError> {
    let db = r.get::<DbPool>()?;
    let config = r.get::<Arc<Config>>()?;
    Ok(Arc::new(UsersGatewaySessionMint::new(
        Arc::new(UserRepository::new(&db)),
        SessionRepository::new(&db),
        ApiKeyLifetime::from_max_days(config.api_key_max_lifetime_days),
        chrono::Duration::seconds(config.jwt_access_token_expiration),
    )))
}

pub(crate) fn sign_in_sessions(r: &Resolved<'_>) -> Result<DynSignInSessions, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(SessionRepository::new(&db)))
}

pub(crate) fn request_authenticator(
    r: &Resolved<'_>,
) -> Result<DynRequestAuthenticator, ProvideError> {
    let db = r.get::<DbPool>()?;
    let config = r.get::<Arc<Config>>()?;
    let revocations = r
        .optional::<DynTokenRevocations>()
        .unwrap_or_else(|| Arc::new(UnavailableRevocations));
    Ok(Arc::new(JwtRequestAuthenticator::new(
        JwtRequestAuthenticatorParts {
            sessions: Arc::new(SessionRepository::new(&db)),
            users: Arc::new(UserService::new(Arc::new(UserRepository::new(&db)))),
            revocations,
            issuer: config.jwt_issuer.clone(),
            keyring: r.get::<VerificationKeyring>()?,
        },
    )))
}

pub(crate) fn rate_limit_buckets(r: &Resolved<'_>) -> Result<DynRateLimitBuckets, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(UserRateLimitBucketRepository::new(&db)))
}

pub fn admin_bootstrap(
    handle: Arc<dyn DatabaseHandle>,
) -> Result<DynAdminBootstrap, AdminBootstrapError> {
    let db = systemprompt_database::pool_from_handle(handle)
        .map_err(|error| AdminBootstrapError(Box::new(error)))?;
    Ok(Arc::new(UsersAdminBootstrap(UserService::new(Arc::new(
        UserRepository::new(&db),
    )))))
}

#[derive(Debug)]
struct UsersAdminBootstrap(UserService);

fn refused(error: crate::UserError) -> AdminBootstrapError {
    AdminBootstrapError(Box::new(error))
}

fn account(user: User) -> AdminAccount {
    AdminAccount {
        active: user.is_active(),
        service_account: user.is_service_account(),
        status: user.status.as_str().to_owned(),
        id: user.id,
        name: user.name,
        email: user.email,
        roles: user.roles,
    }
}

#[async_trait]
impl AdminBootstrap for UsersAdminBootstrap {
    async fn find_by_name(&self, name: &str) -> Result<Option<AdminAccount>, AdminBootstrapError> {
        Ok(self
            .0
            .find_by_name(name)
            .await
            .map_err(refused)?
            .map(account))
    }

    async fn find_by_email(
        &self,
        email: &str,
    ) -> Result<Option<AdminAccount>, AdminBootstrapError> {
        Ok(self
            .0
            .find_by_email(email)
            .await
            .map_err(refused)?
            .map(account))
    }

    async fn create(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> Result<AdminAccount, AdminBootstrapError> {
        self.0
            .create(name, email, full_name, None)
            .await
            .map(account)
            .map_err(refused)
    }

    async fn create_if_absent(
        &self,
        name: &str,
        email: &str,
    ) -> Result<Option<AdminAccount>, AdminBootstrapError> {
        Ok(self
            .0
            .create_if_absent(name, email, None, None)
            .await
            .map_err(refused)?
            .map(account))
    }

    async fn grant_admin(
        &self,
        user: &UserId,
        grant: AdminGrant,
    ) -> Result<AdminAccount, AdminBootstrapError> {
        let admin = UserRole::Admin.as_str().to_owned();
        let roles = match grant {
            AdminGrant::Only => vec![admin],
            AdminGrant::Added => {
                let current = self
                    .0
                    .find_by_id(user)
                    .await
                    .map_err(refused)?
                    .ok_or_else(|| refused(crate::UserError::NotFound(user.clone())))?;
                let mut roles = current.roles;
                if !roles.contains(&admin) {
                    roles.push(admin);
                }
                roles
            },
        };
        self.0
            .assign_roles(user, &roles)
            .await
            .map(account)
            .map_err(refused)
    }

    async fn promote(&self, reference: &str) -> Result<AdminPromotion, AdminBootstrapError> {
        let admin = UserAdminService::new(self.0.clone());
        Ok(
            match admin.promote_to_admin(reference).await.map_err(refused)? {
                PromoteResult::Promoted(user, _) => AdminPromotion::Promoted(account(user)),
                PromoteResult::AlreadyAdmin(user) => AdminPromotion::AlreadyAdmin(account(user)),
                PromoteResult::UserNotFound => AdminPromotion::NotFound,
            },
        )
    }

    async fn session_exists(&self, session: &SessionId) -> Result<bool, AdminBootstrapError> {
        self.0.session_exists(session).await.map_err(refused)
    }
}
