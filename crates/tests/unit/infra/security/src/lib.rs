//! Unit tests for systemprompt-security crate
//!
//! Tests cover:
//! - auth: AuthMode, AuthValidationService, TokenClaims
//! - extraction: TokenExtractor, CookieExtractor, HeaderExtractor, HeaderInjector
//! - jwt: JwtService, AdminTokenParams
//! - services: ScannerDetector
//! - session: SessionGenerator, SessionParams, ValidatedSessionClaims

#![allow(clippy::all)]

#[cfg(test)]
mod auth;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod jwt;
#[cfg(test)]
mod services;
#[cfg(test)]
mod session;
