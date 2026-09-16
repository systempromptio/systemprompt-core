//! OAuth parameter, audience, redirect URI and JWT validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audience;
pub mod client_credentials;
pub mod id_jag;
pub mod jwt;
pub mod oauth_params;
pub mod redirect_uri;
pub mod registration_redirect;

pub use audience::*;
pub use client_credentials::{validate_client_credentials, verify_client_authentication};
pub use id_jag::{
    ClaimPolicy, ID_JAG_TOKEN_TYPE, ID_JAG_TYP, IdJagClaims, IdJagError, validate_claims,
    validate_typ,
};
pub use jwt::*;
pub use oauth_params::*;
pub use redirect_uri::*;
pub use registration_redirect::{
    validate_client_metadata_uri, validate_registration_redirect_uris,
};
