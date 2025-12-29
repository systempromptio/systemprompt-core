//! Unit tests for systemprompt-core-logging crate
//!
//! Tests cover:
//! - LogLevel parsing, display, and as_str methods
//! - LogEntry creation, builder pattern, and validation
//! - LogFilter construction and accessor methods
//! - LoggingError variants and display messages
//! - OutputMode state management and mode switching
//! - RetentionPolicy and RetentionConfig configuration
//! - FieldVisitor and SpanContext field recording

#![allow(clippy::all)]

mod layer;
mod models;
mod services;
