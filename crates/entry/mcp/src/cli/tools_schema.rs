//! Readable parameter-schema view for `plugins mcp tools --schema`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::types::McpToolEntry;

pub(super) fn schema_view(tools: &[McpToolEntry]) -> String {
    let mut lines: Vec<String> = Vec::new();

    for tool in tools {
        lines.push(String::new());
        lines.push(format!("╭─ {}/{}", tool.server, tool.name));

        if let Some(ref desc) = tool.description {
            lines.push(format!("│  {desc}"));
        }

        if let Some(ref schema) = tool.input_schema {
            lines.push("│".to_owned());
            lines.push("│  Parameters:".to_owned());
            lines.extend(schema_properties(schema, "│    "));
        } else {
            lines.push("│  (no parameters)".to_owned());
        }

        lines.push("╰─".to_owned());
    }

    lines.join("\n")
}

// JSON: MCP `inputSchema` — JSON Schema owned by the server.
fn schema_properties(schema: &serde_json::Value, indent: &str) -> Vec<String> {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let required: HashSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map_or_else(HashSet::new, |arr| {
            arr.iter().filter_map(|v| v.as_str()).collect()
        });

    props
        .iter()
        .map(|(name, prop_schema)| {
            let req_marker = if required.contains(name.as_str()) {
                "*"
            } else {
                ""
            };

            let prop_type = prop_schema
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("any");

            let description = prop_schema
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");

            let type_display = prop_schema
                .get("enum")
                .and_then(|e| e.as_array())
                .map_or_else(
                    || prop_type.to_owned(),
                    |values| {
                        let vals: Vec<String> = values
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| format!("\"{s}\"")))
                            .collect();
                        format!("enum[{}]", vals.join("|"))
                    },
                );

            format!("{indent}{name}{req_marker}: {type_display} - {description}")
        })
        .collect()
}
