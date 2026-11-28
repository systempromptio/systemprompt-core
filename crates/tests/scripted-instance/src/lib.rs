//! A scripted stand-in for a systemprompt instance and for the model
//! behind it.
//!
//! A test builds a `ScriptedInstance` from fixtures, starts it in-process
//! on an ephemeral loopback port, points a device at `base_url()`, and
//! afterwards asserts on `requests()`. The same crate ships a binary so a
//! product outside the test process can be pointed at it.
//!
//! It serves discovery and health, the sign-in exchanges, identity, the
//! signed manifest and its key, the model list and profile, both model
//! protocols as scripted streams, the hook client, hook token, telemetry
//! and verdict routes, the heartbeat, device enrolment, plugin bundle
//! files, receipts, session bindings and the Drive sync feed. Every request
//! is recorded, the ones no route answers included. Installation plans are
//! not served: a plan request is recorded and refused.
//!
//! Routes and header names are taken from the linked core crates
//! (`systemprompt_models::ApiPaths`, `systemprompt_identifiers::headers`)
//! and are never spelled in this crate.

mod encode;
mod error;
mod event;
mod file;
mod guard;
mod instance;
mod manifest;
mod model;
mod plugin;
mod record;
mod router;
mod routes;
mod script;
mod state;
mod token;
mod user;
mod verdict;

pub use error::ScriptedInstanceError;
pub use event::Event;
pub use instance::{ScriptedInstance, ScriptedInstanceBuilder};
pub use manifest::{ManifestFault, ManifestFixture};
pub use model::ModelFixture;
pub use plugin::PluginFixture;
pub use record::{RecordedBody, RequestRecord, Route};
pub use script::{
    Aside, Expect, ModelScript, Respond, StopReason, ToolCall, Turn, TurnFault, Usage,
};
pub use user::{TestPat, TestUser};
pub use verdict::{Verdict, VerdictScript};
