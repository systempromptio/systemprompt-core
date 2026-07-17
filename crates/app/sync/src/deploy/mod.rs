//! Cloud deploy orchestration.
//!
//! [`DeployOrchestrator`] sequences the full tenant deploy pipeline —
//! pre-deploy file sync, build-artifact validation, Docker image build and
//! push, secret provisioning (including the `SIGNING_KEY_PEM` transport), and
//! the final deploy call against the cloud API. Every user-facing rendering
//! decision is delegated to a caller-supplied [`DeployProgress`]
//! implementation: the pipeline owns sequencing, the caller owns
//! presentation.
//!
//! Inputs arrive as a typed [`DeployRequest`]; the result is a
//! [`DeployReport`] whose [`DeployOutcome`] distinguishes a dry run from a
//! completed deploy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifacts;
mod orchestrator;
mod pre_sync;
mod progress;
mod request;

pub use artifacts::DeployArtifacts;
pub use orchestrator::DeployOrchestrator;
pub use progress::{DeployEvent, DeployProgress, DeployPrompt};
pub use request::{DeployOptions, DeployOutcome, DeployReport, DeployRequest, PreSyncOptions};
