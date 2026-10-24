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

pub use audience::{validate_any_audience, validate_required_audience, validate_service_access};
pub use client_credentials::{validate_client_credentials, verify_client_authentication};
pub use id_jag::{
    ClaimPolicy, ID_JAG_TOKEN_TYPE, ID_JAG_TYP, IdJagClaims, IdJagError, validate_claims,
    validate_typ,
};
pub use jwt::validate_jwt_token;
pub use oauth_params::{
    CsrfToken, ValidatedClientRegistration, get_audit_user, optional_param, required_param,
    scope_param,
};
pub use redirect_uri::validate_redirect_uri;
pub use registration_redirect::{
    validate_client_metadata_uri, validate_registration_redirect_uris,
};
