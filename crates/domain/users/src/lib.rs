#![allow(missing_debug_implementations)]

pub mod error;
pub mod extension;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod services;

pub use extension::UsersExtension;

pub use error::{Result, UserError};
pub use models::{
    User, UserActivity, UserCountBreakdown, UserExport, UserRole, UserSession, UserStats,
    UserStatus, UserWithSessions,
};
pub use repository::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository, MergeResult,
    UserRepository,
};
pub use services::{
    DemoteResult, PromoteResult, UpdateUserParams, UserAdminService, UserProviderImpl, UserService,
};

pub use systemprompt_traits::auth::{RoleProvider, UserProvider};
