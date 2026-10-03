//! Unit tests for the systemprompt-oauth-issuance crate: client-credentials
//! scope and audience policy, RFC 8693 subject and ID-JAG validation, `act`
//! chain assembly, user-grant permission resolution, request validation, and
//! the grant input and issued-token shapes.

#[cfg(test)]
mod client_credentials;
#[cfg(test)]
mod error;
#[cfg(test)]
mod request;
#[cfg(test)]
mod token_exchange;
#[cfg(test)]
mod user_tokens;
#[cfg(test)]
mod validation;
