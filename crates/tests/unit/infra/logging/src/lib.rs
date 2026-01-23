//! Unit tests for systemprompt-core-logging crate
//!
//! Tests cover:
//! - LogLevel parsing, display, and as_str methods
//! - LogEntry creation, builder pattern, and validation
//! - LogFilter construction and accessor methods
//! - LoggingError variants and display messages
//! - Startup mode state management
//! - RetentionPolicy and RetentionConfig configuration
//! - FieldVisitor and SpanContext field recording
//! - CLI theme types and styling
//! - Trace models for AI and MCP execution tracking

#![allow(clippy::all)]

mod layer;
mod models;
mod services;
mod trace;
