//! Authentication and authorization value types.
//!
//! JWT and cloud claim shapes, the [`AuthenticatedUser`] request
//! identity, [`Permission`] parsing, base role definitions, and the
//! OAuth/PKCE enums (`ResponseType`, `PkceMethod`). `GrantType` lives
//! in `systemprompt_oauth` since it carries RFC 8693 token-exchange.
//! Public functions return [`AuthRequestError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod claims;
pub mod cloud_claims;
pub mod enums;
pub mod permission;
pub mod roles;
pub mod token_authority;
pub mod types;

pub use claims::{
    ActClaim, CustomClaims, JwtClaims, MAX_ACT_CHAIN_DEPTH, RegisteredClaims, TokenInteraction,
};
pub use cloud_claims::CloudAuthClaims;
pub use enums::{
    AccountKind, JwtAudience, RateLimitTier, TokenType, UserRole, UserStatus, UserType,
};
pub use permission::{
    Permission, parse_permissions, parse_roles, permissions_to_string, roles_to_string,
};
pub use roles::BaseRoles;
pub use token_authority::{TokenAuthority, TokenError, TokenResult, UserJwtRequest};
pub use types::{AuthRequestError, AuthenticatedUser, BEARER_PREFIX, PkceMethod, ResponseType};
