//! # systemprompt-users
//!
//! User management for the systemprompt.io AI governance platform. The crate
//! provides:
//!
//! - **6-tier RBAC** — typed `UserRole` and policy-aware promotion/demotion
//!   helpers in [`UserAdminService`].
//! - **Sessions** — lifecycle management for browser, API, and anonymous
//!   sessions including bulk-end and recent-activity queries.
//! - **API keys** — issuance, hashing, and verification via [`ApiKeyService`].
//! - **Device certificates** — enrollment and rotation via
//!   [`DeviceCertService`].
//! - **IP bans** — typed [`BannedIpRepository`] with metadata-aware queries.
//! - **Cleanup job** — purges anonymous users past the retention window.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | _none_  | n/a     | The crate exposes a single feature surface; all modules are compiled unconditionally. The `[package.metadata.docs.rs] all-features = true` setting is retained so future feature additions automatically appear in published docs. |
//!
//! ## Layering
//!
//! `systemprompt-users` is a **domain** crate. It depends downward on
//! `systemprompt-database`, `systemprompt-extension`, `systemprompt-models`,
//! `systemprompt-traits`, `systemprompt-provider-contracts`, and
//! `systemprompt-identifiers`.

#![expect(
    missing_debug_implementations,
    reason = "repositories and services hold pool/transaction handles that do not implement Debug; deriving Debug across this crate would add nothing useful"
)]

pub mod error;
pub(crate) mod extension;
pub mod jobs;
pub(crate) mod models;
pub(crate) mod repository;
pub(crate) mod services;

pub use extension::UsersExtension;

pub use error::{Result, UserError, UserResult};
pub use models::{
    NewApiKey, User, UserActivity, UserApiKey, UserCountBreakdown, UserDeviceCert, UserExport,
    UserRole, UserSession, UserStats, UserStatus, UserWithSessions,
};
pub use repository::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository,
    CreateApiKeyParams, EnrollDeviceCertParams, MergeResult, UserRepository,
};
pub use services::{
    API_KEY_PREFIX, ApiKeyService, DemoteResult, DeviceCertService, EnrollDeviceCertServiceParams,
    IssueApiKeyParams, PromoteResult, UpdateUserParams, UserAdminService, UserProviderImpl,
    UserService,
};

pub use systemprompt_traits::auth::{RoleProvider, UserProvider};
