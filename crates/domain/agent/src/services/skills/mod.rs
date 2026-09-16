//! Skill ingestion for agent prompts, served through the managed-resource
//! resolver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod disk;
pub mod skill;

pub use skill::{SkillMetadata, SkillService};
