//! The per-tool handler contract and the schema shaping every handler's
//! `inputSchema` goes through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::RequestContext;

use crate::schema::McpOutputSchema;

pub trait McpToolHandler: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + JsonSchema + McpOutputSchema + Send;

    fn tool_name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn input_schema(&self) -> JsonValue {
        let schema = schemars::schema_for!(Self::Input);
        match serde_json::to_value(&schema) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize input schema");
                JsonValue::Null
            },
        }
    }

    fn output_schema(&self) -> JsonValue {
        Self::Output::validated_schema()
    }

    fn read_only(&self) -> bool {
        false
    }

    fn tool_definition(&self, server_name: &str) -> Tool {
        let input_obj = object_input_schema(&self.input_schema());
        let output_obj = self
            .output_schema()
            .as_object()
            .cloned()
            .unwrap_or_default();

        let mut tool = Tool::default();
        tool.name = self.tool_name().to_owned().into();
        tool.description = Some(self.description().to_owned().into());
        tool.input_schema = Arc::new(input_obj);
        tool.output_schema = Some(Arc::new(output_obj));
        tool.annotations = self
            .read_only()
            .then(|| rmcp::model::ToolAnnotations::new().read_only(true));
        tool.meta = Some(rmcp::model::MetaObject(crate::capabilities::tool_ui_meta(
            server_name,
            &crate::capabilities::default_tool_visibility(),
        )));
        tool
    }

    fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send;
}

// Why: The MCP contract says a tool's `inputSchema` describes an object, and
// clients hold it to that: Claude Code validates `inputSchema.type ==
// "object"` for every tool and drops the whole server's tool list when one
// fails ("tools fetch failed — Invalid input (at tools.N.inputSchema.type)").
// schemars renders an internally tagged enum as a bare `oneOf` with no root
// `type`, which is exactly that failure; the root gets `type: object` here so
// no handler can ship it by accident.
#[must_use]
pub fn object_input_schema(schema: &JsonValue) -> serde_json::Map<String, JsonValue> {
    let mut obj = schema.as_object().cloned().unwrap_or_default();
    if !obj.contains_key("type") {
        obj.insert("type".to_owned(), JsonValue::String("object".to_owned()));
    }
    obj
}
