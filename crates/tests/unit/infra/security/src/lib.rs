//! Unit tests for systemprompt-security crate
//!
//! Tests cover:
//! - auth: AuthMode, AuthValidationService, TokenClaims
//! - extraction: TokenExtractor, CookieExtractor, HeaderExtractor, HeaderInjector
//! - jwt: JwtService, AdminTokenParams
//! - services: ScannerDetector
//! - session: SessionGenerator, SessionParams, ValidatedSessionClaims

#![allow(clippy::all)]

mod auth;
mod extraction;
mod jwt;
mod services;
mod session;
