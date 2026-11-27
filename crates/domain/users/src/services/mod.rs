//! Service layer for the users domain.
//!
//! Wraps the repositories behind cohesive services: [`UserService`] for
//! account lifecycle and sessions, [`UserAdminService`] for promote/demote
//! workflows, [`ApiKeyService`] for API-key issuance, [`DeviceCertService`]
//! for device-certificate enrollment, and [`DevicePushService`] for a device's
//! push token and client info. [`UserService`] itself implements the
//! cross-crate `UserProvider` trait (see `user::provider`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin_service;
mod api_key_service;
mod api_key_validation;
mod device_cert_service;
mod device_push_service;
mod role_audience;
mod service_accounts;
mod sso_events;
mod user;
mod user_retention;

pub use crate::repository::UpdateUserParams;
pub use admin_service::{DemoteResult, PromoteResult, UserAdminService};
pub use api_key_service::{ApiKeyLifetime, ApiKeyService, EnsuredApiKey, IssueApiKeyParams};
pub use device_cert_service::{
    DEVICE_FINGERPRINT_FOREIGN_USER, DeviceCertService,
    EnrollParams as EnrollDeviceCertServiceParams,
};
pub use device_push_service::{
    ADMIN_DEVICE_LIST_LIMIT, DevicePushError, DevicePushService, MAX_OS_VERSION_LEN,
};
pub use role_audience::UserRoleAudienceResolver;
pub use service_accounts::UsersServiceAccounts;
pub use sso_events::{OUTCOME_REFUSED, OUTCOME_SUCCESS, PostgresAuthEventSink};
pub use user::{PurgedArchive, UserService};
pub(crate) use user_retention::UsersRetention;
