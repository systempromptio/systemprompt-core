//! Compiled input-schema validators for the tools one request offered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use jsonschema::Validator;
use serde_json::Value;

use crate::models::{ArgsCheck, ArgsValidity};

/// Name-keyed validators for one offered tool catalog.
///
/// A missing schema, or one that fails to compile, is kept as an entry with no
/// validator, so a call to that tool reads `SchemaUnusable` rather than
/// `UnknownTool`. When a catalog offers one name twice, the first definition
/// wins. Schemas are compiled without remote `$ref` resolution.
#[derive(Debug, Default)]
pub struct ToolArgsValidator {
    tools: HashMap<String, Option<Validator>>,
}

impl ToolArgsValidator {
    // JSON: tool input schemas are JSON Schema documents supplied by the client.
    pub fn compile<'a>(schemas: impl IntoIterator<Item = (&'a str, Option<&'a Value>)>) -> Self {
        let mut tools = HashMap::new();
        for (name, schema) in schemas {
            if tools.contains_key(name) {
                continue;
            }
            let validator = schema.and_then(|schema| match jsonschema::validator_for(schema) {
                Ok(validator) => Some(validator),
                Err(e) => {
                    tracing::debug!(error = %e, tool_name = %name, "Offered tool schema does not compile");
                    None
                },
            });
            tools.insert(name.to_owned(), validator);
        }
        Self { tools }
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn check_raw(&self, tool_name: &str, raw_input: &str) -> ArgsCheck {
        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return self.check(tool_name, &Value::Object(serde_json::Map::new()));
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(input) => self.check(tool_name, &input),
            Err(_) if !self.tools.contains_key(tool_name) => {
                ArgsCheck::of(ArgsValidity::UnknownTool)
            },
            Err(_) => ArgsCheck::of(ArgsValidity::Unparseable),
        }
    }

    // JSON: tool arguments are whatever object the model emitted.
    pub fn check(&self, tool_name: &str, input: &Value) -> ArgsCheck {
        let Some(entry) = self.tools.get(tool_name) else {
            return ArgsCheck::of(ArgsValidity::UnknownTool);
        };
        let Some(validator) = entry else {
            return ArgsCheck::of(ArgsValidity::SchemaUnusable);
        };
        match validator.validate(input) {
            Ok(()) => ArgsCheck::of(ArgsValidity::Valid),
            Err(error) => ArgsCheck {
                validity: ArgsValidity::Invalid,
                error_path: Some(error.instance_path().as_str().to_owned()),
            },
        }
    }
}
