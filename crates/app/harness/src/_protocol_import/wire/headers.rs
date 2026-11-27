//! Header names the harness sends or reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const CLIENT: &str = "x-systemprompt-client";
pub const HOST: &str = "x-systemprompt-host";
pub const DEVICE_CREDENTIAL: &str = "x-systemprompt-device-credential";
pub const INGESTION_EVENT_ID: &str = "x-ingestion-event-id";
pub const INFERENCE_PROTOCOL: &str = "x-inference-protocol";
pub const CONTEXT_ID: &str = "x-context-id";
pub const DOCUMENT_CONTENT_SHA256: &str = "x-document-content-sha256";
pub const SESSION_ID: &str = "x-session-id";
pub const MEMORY: &str = "x-systemprompt-memory";
pub const MEMORY_IDS: &str = "x-systemprompt-memory-ids";
