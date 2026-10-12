//! Authentication and authorization enumerations.
//!
//! Defines the closed sets the platform reasons over: [`JwtAudience`],
//! [`UserType`], [`TokenType`], [`RateLimitTier`], [`UserRole`], and
//! [`UserStatus`], [`AccountKind`]. [`UserType::from_permissions`] is the
//! single source of truth for the permission-to-type mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod account_kind;
mod audience;
mod caller;
mod user_state;

pub use account_kind::AccountKind;
pub use audience::JwtAudience;
pub use caller::{RateLimitTier, TokenType, UserType};
pub use user_state::{UserRole, UserStatus};
