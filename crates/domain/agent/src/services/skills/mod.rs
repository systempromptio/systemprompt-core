//! Disk-backed skill ingestion: loading skill definitions and injecting their
//! instructions into agent prompts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod skill;
pub mod skill_injector;

pub use skill::{SkillMetadata, SkillService};
pub use skill_injector::SkillInjector;
