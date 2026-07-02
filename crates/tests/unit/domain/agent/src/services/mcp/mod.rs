//! Unit tests for MCP artifact transformer
//!
//! Tests cover:
//! - parse_tool_response (valid JSON, null, empty, malformed)
//! - calculate_fingerprint (determinism, different inputs)
//! - infer_type (schema-based, data-based, tabular, form, chart, envelope
//!   fall-through)
//! - build_metadata (all artifact types, rendering hints)
//! - build_parts (object data, content arrays, error cases)

mod artifact_transformer;
mod task_completion;
mod transformer_full;
