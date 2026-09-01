//! The `mcp.<slug>` remote entries the bridge registers in the user's global
//! `opencode.json`. Every foreign key survives, including MCP servers the user
//! added; only entries pointing at the loopback proxy are bridge-owned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

use crate::gateway::manifest::ManagedMcpServer;
use crate::integration::claude_code_cli::json_io::{object_entry, read_json_object, write_json};
use crate::sync::ApplyError;

use super::super::config::user_config_path;

const MCP_TABLE: &str = "mcp";

pub(super) fn write_mcp_blocks(servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    let path = user_config_path();
    let original = read_json_object(&path)?;
    let mut value = original.clone();

    strip_bridge_servers(&mut value);
    if !servers.is_empty() {
        let bearer = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
            context: "read loopback secret for opencode mcp".into(),
            source: e,
        })?;
        let Some(table) = object_entry(&mut value, MCP_TABLE) else {
            return Ok(());
        };
        for s in servers {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            table.insert(
                slug.clone(),
                json!({
                    "type": "remote",
                    "url": crate::proxy::mcp_url(&slug),
                    "headers": { "Authorization": bearer },
                    "enabled": true,
                }),
            );
        }
    }

    if value == original {
        return Ok(());
    }
    write_json(&path, &Value::Object(value))
}

fn strip_bridge_servers(root: &mut Map<String, Value>) {
    let Some(Value::Object(table)) = root.get_mut(MCP_TABLE) else {
        return;
    };
    let prefix = format!("{}/mcp/", crate::proxy::loopback_origin());
    table.retain(|_, entry| {
        !entry
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|u| u.starts_with(&prefix))
    });
    if table.is_empty() {
        root.remove(MCP_TABLE);
    }
}
