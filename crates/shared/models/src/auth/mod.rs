//! Authentication and authorization value types.
//!
//! JWT and cloud claim shapes, the [`AuthenticatedUser`] request
//! identity, [`Permission`] parsing, base role definitions, and the
//! OAuth/PKCE enums (`ResponseType`, `PkceMethod`). `GrantType` lives
//! in `systemprompt_oauth` since it carries RFC 8693 token-exchange.
//! Public functions return [`AuthError`].

pub mod claims;
pub mod cloud_claims;
pub mod enums;
pub mod permission;
pub mod roles;
pub mod types;

pub use claims::{ActClaim, JwtClaims};
pub use cloud_claims::CloudAuthClaims;
pub use enums::*;
pub use permission::{Permission, parse_permissions, permissions_to_string};
pub use roles::{BaseRole, BaseRoles};
pub use types::{AuthError, AuthenticatedUser, BEARER_PREFIX, PkceMethod, ResponseType};
