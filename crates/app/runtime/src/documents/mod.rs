//! The shared document store, composed for the runtime.
//!
//! [`DocumentsRuntime`] is built once by the [`AppContext`] builder from the
//! context's database, file storage, authz hook, subject source and event
//! router, and reached through [`AppContext::documents`]. [`views`] assembles
//! the transport-neutral views both surfaces answer with and [`refusal`] the
//! one refusal vocabulary; [`mcp`] holds the MCP tool handlers an
//! extension's MCP server binary composes through
//! [`mcp::DocumentsMcpTools`].
//!
//! [`AppContext`]: crate::AppContext
//! [`AppContext::documents`]: crate::AppContext::documents
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod actor;
/// The conversation a write is attributed to, checked against the caller's own.
pub mod attribution;
mod directory;
/// Signed, short-lived document download links.
pub mod download;
/// Signed links an MCP server obtains from the API.
pub mod links;
/// MCP tool handlers over the document service.
pub mod mcp;
/// The refusal vocabulary shared by the MCP tools and the HTTP routes.
pub mod refusal;
mod subjects;
mod unavailable_brain_sink;
mod unavailable_contexts;
mod unavailable_sink;
/// Signed URLs for the resumable upload API.
pub mod upload_links;
/// Read views, access, comments, proposals and write acks as plain structs.
pub mod views;
mod wiring;

pub use directory::UserSubjectDirectory;
pub use subjects::{ROLE_SUBJECT_KIND, USER_SUBJECT_KIND, active_user_display, subject_display};
pub use unavailable_brain_sink::BrainUnavailableSink;
pub(crate) use unavailable_brain_sink::admitted_brain_sink;
pub use unavailable_contexts::ContextsUnavailable;
pub use unavailable_sink::NotificationsUnavailableSink;
pub use wiring::{DocumentsRuntime, DocumentsRuntimeParts};
