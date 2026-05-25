use systemprompt_logging::CliService;

use super::types::McpToolEntry;

pub(crate) fn print_schema_view(tools: &[McpToolEntry]) {
    CliService::section("MCP Tool Schemas");

    for tool in tools {
        CliService::info("");
        CliService::info(&format!("╭─ {}/{}", tool.server, tool.name));

        if let Some(ref desc) = tool.description {
            CliService::info(&format!("│  {}", desc));
        }

        if let Some(ref schema) = tool.input_schema {
            CliService::info("│");
            CliService::info("│  Parameters:");
            print_schema_properties(schema, "│    ");
        } else {
            CliService::info("│  (no parameters)");
        }

        CliService::info("╰─");
    }
}

fn print_schema_properties(schema: &serde_json::Value, indent: &str) {
    let properties = schema.get("properties").and_then(|p| p.as_object());
    let required = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map_or_else(std::collections::HashSet::new, |arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<std::collections::HashSet<_>>()
        });

    if let Some(props) = properties {
        for (name, prop_schema) in props {
            let is_required = required.contains(name.as_str());
            let req_marker = if is_required { "*" } else { "" };

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
                    || prop_type.to_string(),
                    |values| {
                        let vals: Vec<String> = values
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| format!("\"{}\"", s)))
                            .collect();
                        format!("enum[{}]", vals.join("|"))
                    },
                );

            CliService::info(&format!(
                "{}{}{}: {} - {}",
                indent, name, req_marker, type_display, description
            ));
        }
    }
}
