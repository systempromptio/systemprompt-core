#![allow(missing_debug_implementations)]

pub(crate) mod error;
pub(crate) mod extension;
pub mod jobs;
pub(crate) mod models;
pub(crate) mod repository;
pub(crate) mod services;

pub use extension::UsersExtension;

pub use error::{Result, UserError};
pub use models::{
    NewApiKey, User, UserActivity, UserApiKey, UserCountBreakdown, UserExport, UserRole,
    UserSession, UserStats, UserStatus, UserWithSessions,
};
pub use repository::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository,
    CreateApiKeyParams, MergeResult, UserRepository,
};
pub use services::{
    API_KEY_PREFIX, ApiKeyService, DemoteResult, IssueApiKeyParams, PromoteResult,
    UpdateUserParams, UserAdminService, UserProviderImpl, UserService,
};

pub use systemprompt_traits::auth::{RoleProvider, UserProvider};
