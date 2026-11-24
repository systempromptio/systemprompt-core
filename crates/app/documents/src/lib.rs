//! `systemprompt-documents` — the shared document store, composed.
//!
//! [`DocumentsRuntime`] is built once per host by the `documents` member
//! ([`extension::DocumentsExtension`]) from the resolved kernel state and
//! bound into the `DocumentsHandlesSlot` it provides, which every consumer
//! reads. [`views`] assembles the transport-neutral views both surfaces
//! answer with and [`refusal`] the one refusal vocabulary; [`mcp`] holds
//! the MCP tool handlers a server composes through
//! [`mcp::DocumentsMcpTools`]; [`tokens`] holds the two narrow token views
//! the member confers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod actor;
/// The conversation a write is attributed to, checked against the caller's own.
pub mod attribution;
mod directory;
/// The `endpoints.documents` block of the root discovery document.
pub mod discovery;
/// Signed, short-lived document download links.
pub mod download;
/// The documents member.
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
/// Signed links an MCP server obtains from the API.
pub mod links;
/// MCP tool handlers over the document service.
pub mod mcp;
mod provides;
/// The refusal vocabulary shared by the MCP tools and the HTTP routes.
pub mod refusal;
mod runtime_handles;
mod subjects;
/// The narrow token views the member confers.
pub mod tokens;
mod unavailable_brain_sink;
mod unavailable_contexts;
mod unavailable_sink;
/// Signed URLs for the resumable upload API.
pub mod upload_links;
/// Read views, access, comments, proposals and write acks as plain structs.
pub mod views;
mod wiring;

pub use directory::UserSubjectDirectory;
pub use runtime_handles::bound_handles;
pub use subjects::{ROLE_SUBJECT_KIND, USER_SUBJECT_KIND, active_user_display, subject_display};
pub use unavailable_brain_sink::BrainUnavailableSink;
pub use unavailable_contexts::ContextsUnavailable;
pub use unavailable_sink::NotificationsUnavailableSink;
pub use wiring::{DocumentsRuntime, DocumentsRuntimeParts};
