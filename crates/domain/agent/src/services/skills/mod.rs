//! Disk-backed skill ingestion: loading skill definitions and injecting their
//! instructions into agent prompts.

pub mod skill;
pub mod skill_injector;

pub use skill::{SkillMetadata, SkillService};
pub use skill_injector::SkillInjector;
