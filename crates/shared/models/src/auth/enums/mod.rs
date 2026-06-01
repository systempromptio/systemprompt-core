//! Authentication and authorization enumerations.
//!
//! Defines the closed sets the platform reasons over: [`JwtAudience`],
//! [`UserType`], [`TokenType`], [`RateLimitTier`], [`UserRole`], and
//! [`UserStatus`]. [`UserType::from_permissions`] is the single source of
//! truth for the permission-to-type mapping.

mod audience;
mod caller;
mod user_state;

pub use audience::JwtAudience;
pub use caller::{RateLimitTier, TokenType, UserType};
pub use user_state::{UserRole, UserStatus};
