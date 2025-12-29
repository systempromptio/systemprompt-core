//! Unit tests for systemprompt-core-security crate
//!
//! Tests cover:
//! - auth: AuthMode, AuthValidationService, TokenClaims
//! - extraction: TokenExtractor, CookieExtractor, HeaderInjector
//! - jwt: JwtService, AdminTokenParams
//! - services: ScannerDetector

#![allow(clippy::all)]

mod auth;
mod extraction;
mod jwt;
mod services;
