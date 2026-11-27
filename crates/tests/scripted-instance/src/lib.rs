//! A scripted stand-in for a systemprompt instance and for the model
//! behind it.
//!
//! A test builds a `ScriptedInstance` from fixtures, starts it in-process
//! on an ephemeral loopback port, points a device at `base_url()`, and
//! afterwards asserts on `requests()`. The same crate ships a binary so a
//! product outside the test process can be pointed at it.
//!
//! This revision fixes the public interface only. The router serves the
//! health route and nothing else; no route records a request, signs a
//! manifest or encodes a stream. Operations that need that behaviour return
//! `ScriptedInstanceError::NotImplemented`.
//!
//! Routes and header names are taken from the linked core crates
//! (`systemprompt_models::ApiPaths`, `systemprompt_identifiers::headers`)
//! and are never spelled in this crate.

mod error;
mod event;
mod file;
mod instance;
mod manifest;
mod model;
mod record;
mod router;
mod script;
mod state;
mod user;
mod verdict;

pub use error::ScriptedInstanceError;
pub use event::Event;
pub use instance::{ScriptedInstance, ScriptedInstanceBuilder};
pub use manifest::{ManifestFault, ManifestFixture};
pub use model::ModelFixture;
pub use record::{RecordedBody, RequestRecord, Route};
pub use script::{Expect, ModelScript, Respond, StopReason, ToolCall, Turn, TurnFault, Usage};
pub use user::{TestPat, TestUser};
pub use verdict::{Verdict, VerdictScript};
