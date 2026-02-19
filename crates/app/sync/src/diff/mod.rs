mod agents;
mod content;
mod skills;

pub use agents::AgentsDiffCalculator;
pub use content::ContentDiffCalculator;
pub use skills::SkillsDiffCalculator;

use crate::models::{DiskAgent, DiskSkill};
use sha2::{Digest, Sha256};
use systemprompt_agent::models::{Agent, Skill};

pub fn compute_content_hash(body: &str, title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn compute_skill_hash(skill: &DiskSkill) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill.name.as_bytes());
    hasher.update(skill.description.as_bytes());
    hasher.update(skill.instructions.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn compute_db_skill_hash(skill: &Skill) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill.name.as_bytes());
    hasher.update(skill.description.as_bytes());
    hasher.update(skill.instructions.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn compute_agent_hash(agent: &DiskAgent) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.name.as_bytes());
    hasher.update(agent.display_name.as_bytes());
    hasher.update(agent.description.as_bytes());
    if let Some(sp) = &agent.system_prompt {
        hasher.update(sp.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub(crate) fn compute_db_agent_hash(agent: &Agent) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.name.as_bytes());
    hasher.update(agent.display_name.as_bytes());
    hasher.update(agent.description.as_bytes());
    if let Some(sp) = &agent.system_prompt {
        hasher.update(sp.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
