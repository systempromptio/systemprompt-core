//! Disk ↔ database sync drivers per content kind.
//!
//! Each submodule (agents, skills, content) owns one direction-agnostic
//! orchestrator that reuses the diff calculators from [`crate::diff`].

mod access_control_sync;
mod agents_sync;
mod content_sync;
mod skills_sync;

pub use access_control_sync::AccessControlLocalSync;
pub use agents_sync::AgentsLocalSync;
pub use content_sync::{ContentDiffEntry, ContentLocalSync};
pub use skills_sync::SkillsLocalSync;
