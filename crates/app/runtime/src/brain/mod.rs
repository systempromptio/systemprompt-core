//! The brain composed for the runtime.
//!
//! When the profile enables the brain, the [`AppContext`] builder assembles
//! one [`BrainHandle`]: the brain repositories, the outbox sink the document
//! store emits brain events through, the person-facing [`BrainService`],
//! and the [`BrainRecallService`] the gateway's recall stage calls. The
//! capture consumer needs a model client the API composes from the gateway,
//! so the API server builds it through [`BrainHandle::start_consumer`] and
//! runs it as a supervised runner service. With the brain disabled nothing
//! is composed: [`AppContext::brain`] is `None` and the document store keeps
//! the unavailable sink, so producers record nothing.
//!
//! The core seams are appended after the extensions' ones:
//! [`DocumentSourceContent`] reads document and comment sources for
//! extraction, and [`DocumentSourceAccess`] decides whether a recalled
//! fact's source document is readable by the reader and the acting agent.
//!
//! [`AppContext`]: crate::AppContext
//! [`AppContext::brain`]: crate::AppContext::brain
//! [`BrainService`]: systemprompt_brain::service::BrainService
//! [`BrainRecallService`]: systemprompt_brain::recall::BrainRecallService
//!
//! [`mcp`] holds the MCP tools an agent inspects team memory and proposes
//! facts through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handle;
pub mod mcp;
mod source_access;
mod source_content;

pub use handle::{BrainHandle, BrainHandleParts};
pub use source_access::DocumentSourceAccess;
pub use source_content::DocumentSourceContent;

pub const BRAIN_ACTOR: &str = "brain";
