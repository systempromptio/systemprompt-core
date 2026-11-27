//! The tool catalogue across servers: each tool gets a qualified name that is
//! unique, safe for model APIs and stable for a given set of servers and tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::mcp::types::{ToolInfo, empty_object_schema};

pub const MAX_QUALIFIED_NAME_LEN: usize = 64;

/// A server tool as the model sees it.
///
/// Its qualified name is `server_tool` with every character outside
/// `[a-zA-Z0-9_-]` replaced by `_`, cut to [`MAX_QUALIFIED_NAME_LEN`], with a
/// numeric suffix when two tools would otherwise collide.
#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub server: String,
    pub name: String,
    pub qualified_name: String,
    pub description: String,
    pub input_schema: Value,
}

pub fn qualify(server: &str, tool: &str) -> String {
    let mut name = String::with_capacity(server.len() + tool.len() + 1);
    name.push_str(&sanitize(server));
    name.push('_');
    name.push_str(&sanitize(tool));
    name.truncate(MAX_QUALIFIED_NAME_LEN);
    name
}

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn unique(base: String, taken: &HashSet<String>) -> String {
    if !taken.contains(&base) {
        return base;
    }
    let mut n = 2usize;
    loop {
        let suffix = format!("_{n}");
        let mut candidate = base.clone();
        candidate.truncate(MAX_QUALIFIED_NAME_LEN.saturating_sub(suffix.len()));
        candidate.push_str(&suffix);
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

// JSON: a tool's input schema is JSON Schema, handed to the model as an object
// schema.
fn normalize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(object) => {
            let mut object: Map<String, Value> = object.clone();
            object
                .entry("type")
                .or_insert_with(|| Value::from("object"));
            Value::Object(object)
        },
        _ => empty_object_schema(),
    }
}

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    tools: Vec<McpTool>,
    index: HashMap<String, usize>,
}

impl Catalog {
    pub(crate) fn build<'a>(servers: impl IntoIterator<Item = (&'a str, &'a [ToolInfo])>) -> Self {
        let mut tools = Vec::new();
        let mut taken = HashSet::new();
        for (server, listed) in servers {
            for tool in listed {
                let qualified_name = unique(qualify(server, &tool.name), &taken);
                taken.insert(qualified_name.clone());
                tools.push(McpTool {
                    server: server.to_owned(),
                    name: tool.name.clone(),
                    qualified_name,
                    description: tool
                        .description
                        .clone()
                        .or_else(|| tool.title.clone())
                        .unwrap_or_default(),
                    input_schema: normalize_schema(&tool.input_schema),
                });
            }
        }
        let index = tools
            .iter()
            .enumerate()
            .map(|(i, tool)| (tool.qualified_name.clone(), i))
            .collect();
        Self { tools, index }
    }

    pub(crate) fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub(crate) fn get(&self, qualified_name: &str) -> Option<&McpTool> {
        self.index
            .get(qualified_name)
            .and_then(|&i| self.tools.get(i))
    }
}
