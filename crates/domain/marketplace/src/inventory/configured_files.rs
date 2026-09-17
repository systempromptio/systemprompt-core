//! Captures the on-disk (or inline-configured) files of an inventory entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_models::services::ServicesConfig;

use super::InventoryEntry;
use super::catalog::invalid;
use crate::managed::{AssetFile, ManagedError, Result, RevisionFiles, capture_inventory_files};

pub(super) fn configured_files(
    root: &Path,
    entry: &InventoryEntry,
    services: &ServicesConfig,
) -> Result<RevisionFiles> {
    let inline = match entry.kind.as_str() {
        "agent" => Some(
            serde_yaml::to_string(
                services
                    .agents
                    .get(&entry.resource_key)
                    .ok_or_else(|| invalid("Configured agent disappeared"))?,
            )
            .map_err(|error| {
                ManagedError::Invalid(format!("Agent configuration cannot be captured: {error}"))
            })?,
        ),
        "mcp" => Some(
            serde_yaml::to_string(
                services
                    .mcp_servers
                    .get(&entry.resource_key)
                    .ok_or_else(|| invalid("Configured MCP server disappeared"))?,
            )
            .map_err(|error| {
                ManagedError::Invalid(format!("MCP configuration cannot be captured: {error}"))
            })?,
        ),
        _ => None,
    };
    if let Some(config) = inline {
        let files = RevisionFiles(BTreeMap::from([(
            "config.yaml".to_owned(),
            AssetFile {
                bytes: config.into_bytes(),
                media_type: "application/yaml".to_owned(),
                executable: false,
            },
        )]));
        files.validate()?;
        Ok(files)
    } else {
        capture_inventory_files(
            root,
            entry
                .configured_key
                .as_deref()
                .ok_or_else(|| invalid("Missing authoring path"))?,
        )
    }
}
