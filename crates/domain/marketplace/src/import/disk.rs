//! Serialisable mirrors of the loader's on-disk descriptors.
//!
//! `DiskSkillConfig` and `DiskHookConfig` in `systemprompt-models` are
//! deserialise-only — they describe what the loader reads, not what anything
//! writes. The importer is the first producer of those files, so it carries the
//! matching write shapes here; the round-trip test is what keeps the two in
//! step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::HookId;
use systemprompt_models::services::hooks::{HookCategory, HookEvent};

#[derive(Debug, Clone, Serialize)]
pub(super) struct SkillDoc {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub file: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_category: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct HookDoc {
    pub id: HookId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub event: HookEvent,
    pub matcher: String,
    pub command: String,
    #[serde(rename = "async")]
    pub is_async: bool,
    pub category: HookCategory,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}
