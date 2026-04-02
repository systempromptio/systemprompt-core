//! Unit tests for API middleware components
//!
//! Tests cover:
//! - Bot detection from user agent strings
//! - Scanner request detection from paths and user agents
//! - Trailing slash redirect logic
//! - Auth config public path matching
//! - Content negotiation accept header parsing
//! - Security headers config defaults
//! - Rate limit config construction and tier multipliers
//! - Session tracking skip logic
//! - Context requirement display and defaults
//! - CORS error variants

mod auth_config;
mod bot_detection_functions;
mod bot_detector;
mod content_negotiation;
mod context_requirement;
mod cors_config;
mod rate_limit_config;
mod security_headers;
mod session_tracking;
mod should_redirect;
mod trailing_slash;
