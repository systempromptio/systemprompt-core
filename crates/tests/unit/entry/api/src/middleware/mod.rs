//! Unit tests for API middleware components
//!
//! Tests cover:
//! - Bot detection from user agent strings
//! - Scanner request detection from paths and user agents
//! - Trailing slash redirect logic
//! - Header context extraction

mod bot_detector;
mod trailing_slash;
