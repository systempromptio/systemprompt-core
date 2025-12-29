//! Unit tests for API middleware components
//!
//! Tests cover:
//! - JWT token extraction from headers and cookies
//! - Bot detection from user agent strings
//! - Scanner request detection from paths and user agents
//! - Trailing slash redirect logic
//! - Header context extraction

mod bot_detector;
mod jwt_token;
mod trailing_slash;
