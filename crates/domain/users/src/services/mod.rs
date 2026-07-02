//! Service layer for the users domain.
//!
//! Wraps the repositories behind cohesive services: [`UserService`] for
//! account lifecycle and sessions, [`UserAdminService`] for promote/demote
//! workflows, [`ApiKeyService`] for API-key issuance, and [`DeviceCertService`]
//! for device-certificate enrollment. [`UserService`] itself implements the
//! cross-crate `UserProvider` trait (see `user::provider`).

mod admin_service;
mod api_key_service;
mod device_cert_service;
mod user;

pub use crate::repository::UpdateUserParams;
pub use admin_service::{DemoteResult, PromoteResult, UserAdminService};
pub use api_key_service::{API_KEY_PREFIX, ApiKeyService, IssueApiKeyParams};
pub use device_cert_service::{DeviceCertService, EnrollParams as EnrollDeviceCertServiceParams};
pub use user::UserService;
