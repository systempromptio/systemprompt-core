//! The creating tool: `documents_create`, a document from a template with
//! optional first content (`{model}` or `{markdown}`) and edit ops applied
//! in the same first revision, authored by the agent with the MCP execution as
//! its origin. The answer carries the lines the first revision wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::{ModelViewV2, RICH_TEXT_VERSION, RichText, markdown};
use systemprompt_files::documents::service::CreateRequest;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::DocumentsMcpTools;
use super::inputs::CreateInput;
use super::media::{image_refs, resolve_media};
use super::outputs::CreateOutput;
use super::refusal::refusal;
use super::support::{agent_author, caller, tool_error};
use crate::documents::refusal::RefusalCode;
use crate::documents::views::written_lines;

const CONTENT_SHAPES: &str = "content is exactly {\"model\": <ModelViewV2>} or {\"markdown\": \
                              \"<text>\"} (schema: GET /api/v1/documents/schema)";

// JSON: the untyped create content, checked here against its two shapes.
pub fn create_content(content: Option<serde_json::Value>) -> Result<Option<RichText>, McpError> {
    let invalid = |detail: String| refusal(RefusalCode::InvalidParams, detail);
    let Some(content) = content else {
        return Ok(None);
    };
    let entry = content
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.iter().next());
    match entry {
        Some((key, serde_json::Value::String(markdown))) if key == "markdown" => {
            Ok(Some(markdown::import(markdown)))
        },
        Some((key, value)) if key == "model" => {
            let view: ModelViewV2 = serde_json::from_value(value.clone()).map_err(|source| {
                tool_error(DocumentsError::Codec(CodecError::Payload {
                    what: "content.model",
                    source,
                }))
            })?;
            if view.version > RICH_TEXT_VERSION || view.kind != "rich_text" {
                return Err(invalid(format!(
                    "content.model is {} v{}; this server reads rich_text up to v{RICH_TEXT_VERSION}; \
                     {CONTENT_SHAPES}",
                    view.kind, view.version
                )));
            }
            Ok(Some(view.model))
        },
        _ => Err(invalid(CONTENT_SHAPES.to_owned())),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CreateHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for CreateHandler<'_> {
    type Input = CreateInput;
    type Output = CreateOutput;

    fn tool_name(&self) -> &'static str {
        "documents_create"
    }

    fn description(&self) -> &'static str {
        "Create from a template. Guide: model."
    }

    async fn handle(
        &self,
        input: CreateInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(CreateOutput, String), McpError> {
        let who = caller(ctx)?;
        let applied = input.ops.len();
        let content = create_content(input.content)?;
        let refs = image_refs(content.as_ref(), &input.ops);
        let media = resolve_media(self.tools, ctx, refs, None).await?;
        let (document, revision) = self
            .tools
            .service()
            .create_from_template(
                who,
                CreateRequest {
                    template_id: input.template_id,
                    folder: input.folder_id,
                    name: input.name,
                    params: TemplateParams {
                        values: input.params,
                    },
                    ops: input.ops,
                    author: agent_author(self.tools, ctx, exec_id, input.context_id.as_ref())
                        .await?,
                    message: input.message,
                    content,
                    media,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let (_, projection) = self
            .tools
            .service()
            .project(who, &document.id, Some(&revision.id), ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let written = written_lines(None, &projection);
        let summary = format!(
            "created {} at revision {}: {applied} ops, {} lines written",
            document.name,
            revision.id,
            written.len()
        );
        Ok((
            CreateOutput {
                document_id: document.id,
                revision_id: revision.id,
                format: document.format,
                name: document.name,
                written,
            },
            summary,
        ))
    }
}
