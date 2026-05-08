//! Pure-function diff calculators for agents and content.
//!
//! Each calculator hashes the disk-side and database-side representations
//! and emits a structured diff (`added`/`modified`/`removed`/`unchanged`)
//! without mutating either side.

mod agents;
mod content;

pub use agents::AgentsDiffCalculator;
pub use content::ContentDiffCalculator;

use crate::models::DiskAgent;
use sha2::{Digest, Sha256};
use systemprompt_agent::models::Agent;

pub fn compute_content_hash(body: &str, title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) fn compute_agent_hash(agent: &DiskAgent) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.name.as_bytes());
    hasher.update(agent.display_name.as_bytes());
    hasher.update(agent.description.as_bytes());
    if let Some(sp) = &agent.system_prompt {
        hasher.update(sp.as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub(crate) fn compute_db_agent_hash(agent: &Agent) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.name.as_bytes());
    hasher.update(agent.display_name.as_bytes());
    hasher.update(agent.description.as_bytes());
    if let Some(sp) = &agent.system_prompt {
        hasher.update(sp.as_bytes());
    }
    hex::encode(hasher.finalize())
}
