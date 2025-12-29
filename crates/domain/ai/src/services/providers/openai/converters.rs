use anyhow::{anyhow, Result};

use crate::models::ai::ResponseFormat;
use crate::models::providers::openai::{
    OpenAiFunction, OpenAiJsonSchema, OpenAiResponseFormat, OpenAiTool,
};
use crate::models::tools::McpTool;

pub fn convert_tools(tools: Vec<McpTool>) -> Result<Vec<OpenAiTool>> {
    tools
        .into_iter()
        .map(|tool| {
            let input_schema = tool
                .input_schema
                .ok_or_else(|| anyhow!("Tool '{}' missing input_schema", tool.name))?;

            Ok(OpenAiTool {
                r#type: "function".to_string(),
                function: OpenAiFunction {
                    name: tool.name,
                    description: tool.description,
                    parameters: input_schema,
                },
            })
        })
        .collect()
}

pub fn convert_response_format(format: &ResponseFormat) -> Result<Option<OpenAiResponseFormat>> {
    match format {
        ResponseFormat::Text => Ok(None),
        ResponseFormat::JsonObject => Ok(Some(OpenAiResponseFormat::JsonObject)),
        ResponseFormat::JsonSchema {
            schema,
            name,
            strict,
        } => {
            let schema_name = name
                .clone()
                .ok_or_else(|| anyhow!("JSON schema response format requires a name"))?;

            Ok(Some(OpenAiResponseFormat::JsonSchema {
                json_schema: OpenAiJsonSchema {
                    name: schema_name,
                    schema: schema.clone(),
                    strict: *strict,
                },
            }))
        },
    }
}
