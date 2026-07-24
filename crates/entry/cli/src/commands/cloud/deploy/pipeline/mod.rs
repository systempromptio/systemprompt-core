//! Cloud deploy orchestration.
//!
//! [`DeployOrchestrator`] sequences the tenant deploy pipeline —
//! build-artifact validation, Docker image build and push, secret
//! provisioning (including the `SIGNING_KEY_PEM` transport), and the final
//! deploy call against the cloud API. Every user-facing rendering decision is
//! delegated to a caller-supplied [`DeployProgress`] implementation: the
//! pipeline owns sequencing, the caller owns presentation.
//!
//! Deploys are stateless container rebuilds: runtime files created inside the
//! previous container are not preserved. `systemprompt cloud backup` exists
//! for operators who want a copy first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifacts;
mod orchestrator;
mod progress;
mod request;

pub use artifacts::DeployArtifacts;
pub use orchestrator::DeployOrchestrator;
pub use progress::{DeployEvent, DeployProgress};
pub use request::{DeployOptions, DeployReport, DeployRequest};
