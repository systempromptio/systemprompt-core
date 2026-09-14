//! Evaluator worker configuration: which worker identity this replica claims
//! assignments under and the pinned images it runs them with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::EvalWorkerId;

/// Present only on replicas that run evaluator assignments; the supervisor
/// job is idle without it.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorConfig {
    pub worker_id: EvalWorkerId,
    pub client_image: String,
    pub relay_image: String,
    pub control_network: String,
    #[serde(default = "default_docker")]
    pub docker: PathBuf,
    #[serde(default = "default_workspace_root")]
    pub workspace_root: PathBuf,
}

fn default_docker() -> PathBuf {
    PathBuf::from("/usr/bin/docker")
}

fn default_workspace_root() -> PathBuf {
    PathBuf::from("/var/lib/systemprompt/evaluator")
}
