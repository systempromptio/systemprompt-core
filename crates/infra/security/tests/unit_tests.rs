//! Unit tests for systemprompt-core-security crate
//!
//! Tests cover:
//! - auth: AuthMode, AuthValidationService, TokenClaims
//! - extraction: TokenExtractor, CookieExtractor, HeaderInjector
//! - jwt: JwtService, AdminTokenParams
//! - services: ScannerDetector

#![allow(clippy::all)]

#[path = "../../../tests/unit/infra/security/auth/mod.rs"]
mod auth;

#[path = "../../../tests/unit/infra/security/extraction/mod.rs"]
mod extraction;

#[path = "../../../tests/unit/infra/security/jwt/mod.rs"]
mod jwt;

#[path = "../../../tests/unit/infra/security/services/mod.rs"]
mod services;
