//! Unit tests for the systemprompt-oauth-issuance crate: client-credentials
//! scope and audience policy, RFC 8693 subject and ID-JAG validation, `act`
//! chain assembly, user-grant permission resolution, request validation, and
//! the grant input and issued-token shapes, the SSO driver end to end, the
//! generic OIDC provider, and the `identity` config plane's declaration loading
//! and rendering.

#[cfg(test)]
mod client_credentials;
#[cfg(test)]
mod drive_sync;
#[cfg(test)]
mod error;
#[cfg(test)]
mod identity_plane;
#[cfg(test)]
mod request;
#[cfg(test)]
mod sso_cookie;
#[cfg(test)]
mod sso_driver;
#[cfg(test)]
mod sso_oidc;
#[cfg(test)]
mod sso_permissions;
#[cfg(test)]
mod sso_resolution;
#[cfg(test)]
mod token_exchange;
#[cfg(test)]
mod user_tokens;
#[cfg(test)]
mod validation;
