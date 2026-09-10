//! Rule configuration and disk-descriptor model.
//!
//! A *rule* is a markdown instruction file a plugin ships alongside its skills.
//! [`DiskRuleConfig`] is the per-rule on-disk descriptor at
//! `rules/<id>/config.yaml`, naming the markdown file that carries the rule
//! text. Unlike the skill and hook descriptors this one is also serialisable:
//! the marketplace importer writes these files from an Anthropic `rules/*.md`
//! tree, so read and write share one shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::PluginRuleId;

const fn default_true() -> bool {
    true
}

pub const RULE_CONFIG_FILENAME: &str = "config.yaml";

pub const DEFAULT_RULE_CONTENT_FILE: &str = "index.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskRuleConfig {
    pub id: PluginRuleId,
    pub name: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file: String,
}

impl DiskRuleConfig {
    #[must_use]
    pub fn content_file(&self) -> &str {
        if self.file.is_empty() {
            DEFAULT_RULE_CONTENT_FILE
        } else {
            &self.file
        }
    }
}
