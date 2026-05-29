//! Unit tests for systemprompt-core-oauth crate
//!
//! Tests cover:
//! - OAuth types (GrantType, PkceMethod, ResponseType, etc.)
//! - OAuthClient model and validation
//! - Dynamic registration request/response
//! - CIMD metadata validation
//! - Token generation and validation
//! - JWT extraction from headers/cookies
//! - OAuth parameter validation
//! - Redirect URI validation
//! - Audience validation

#[cfg(test)]
mod constants;

#[cfg(test)]
mod error;

#[cfg(test)]
mod models;

#[cfg(test)]
mod oauth_parse_error;

#[cfg(test)]
mod pagination;

#[cfg(test)]
mod services;
