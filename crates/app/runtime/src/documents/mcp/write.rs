//! Writing tools: `documents_create` and `documents_edit`.
//!
//! Every edit is a commit applied to the head, authored by an agent with the
//! MCP execution as its origin and the request's session, context, trace
//! and task as its attribution. Each anchored op guards only the node it
//! touches: a node that changed at the head is refused as `guard_mismatch`
//! with its current text, and an edit that landed while the head moved past
//! `base_revision` answers `merged`. A `restore` whose base is not the head
//! is refused as `stale_base`. An `op_id` makes the call idempotent: the
//! commit stores its acknowledgement in the same transaction, and a repeat
//! of the same request returns it with `replayed`. Every write answers with
//! the lines it wrote, diffed against the parent revision, so the next edit
//! anchors on the ack. An edit whose mode resolves to
//! `propose` is written as a proposal instead (see `proposals`), and when
//! the caller did not ask for one (an agent capped at `suggest`) the summary
//! says it was converted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use rmcp::ErrorData as McpError;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::{ModelViewV2, RICH_TEXT_VERSION, RichText, markdown};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, CreateRequest, EditModeRequest,
};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentOpId, DocumentRevisionId, McpExecutionId,
};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ActorKind, AwarenessCursor, AwarenessMode, AwarenessState, DocumentAwarenessEvent, EditMode,
    EditOp, PresenceBeacon, PresenceSource, PresenceState,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::DocumentsMcpTools;
use super::inputs::{CreateInput, EditInput};
use super::media::{image_refs, resolve_media};
use super::outputs::CreateOutput;
use super::proposals::propose_ops;
use super::refusal::refusal;
use super::support::{agent_author, caller, stale_base, tool_error};
use crate::documents::refusal::RefusalCode;
use crate::documents::views::{EditOutput, edit_ack, written_lines};

const CONVERTED: &str = "Written as a proposal, not landed: agents may only suggest on this \
                         document (its agent cap is suggest, or its folder's agent edit policy \
                         is propose), or you hold comment but not edit";

pub(super) struct EditCall<'a> {
    pub(super) tools: &'a DocumentsMcpTools,
    pub(super) who: &'a AuthenticatedUser,
    pub(super) ctx: &'a RequestContext,
    pub(super) exec_id: &'a McpExecutionId,
    pub(super) document: DocumentId,
    pub(super) base: Option<DocumentRevisionId>,
    pub(super) ops: Vec<EditOp>,
    pub(super) mode: Option<EditMode>,
    pub(super) op_id: Option<DocumentOpId>,
    pub(super) message: Option<String>,
    pub(super) comments: CommentPolicy,
    pub(super) verbose: bool,
}

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

/// Files the agent's own presence on the document (source `agent`, so a
/// console shows the person and their assistant side by side) and places its
/// caret on the first block it wrote, so open tabs see where it is working.
/// Neither is part of the write: a failure is logged and the ack stands.
async fn announce_agent(call: &EditCall<'_>, output: &EditOutput) {
    let agent = call.ctx.agent_name().to_string();
    let service = call.tools.service();
    let beacon = PresenceBeacon {
        document_id: call.document.clone(),
        state: PresenceState::Editing,
        source_kind: PresenceSource::Agent,
        source_ref: agent.clone(),
        cursor: None,
    };
    if let Err(error) = service
        .touch_presence(call.who, beacon, call.ctx.trace_id())
        .await
    {
        tracing::warn!(document_id = %call.document, %error, "agent presence not recorded");
    }
    let Some(block) = output
        .written
        .first()
        .and_then(|line| line.split(':').next())
        .and_then(|id| DocumentNodeId::try_new(id).ok())
    else {
        return;
    };
    let cursor = AwarenessCursor {
        block,
        anchor: String::new(),
        head: String::new(),
    };
    let event = DocumentAwarenessEvent {
        document_id: call.document.clone(),
        user_id: call.who.id.clone(),
        display_name: format!("{agent} for {}", call.who.username),
        state: AwarenessState {
            session_id: None,
            color: AGENT_CARET_COLOR.to_owned(),
            mode: AwarenessMode::Editing,
            cursor: Some(cursor),
            selection: Vec::new(),
            viewport: None,
            agent: Some(agent),
        },
        expires_at: Utc::now() + chrono::Duration::seconds(AGENT_CARET_SECS),
    };
    if let Err(error) = service.events().awareness(&event).await {
        tracing::warn!(document_id = %call.document, %error, "agent caret not announced");
    }
}

const AGENT_CARET_COLOR: &str = "tone-agent";
const AGENT_CARET_SECS: i64 = 30;

async fn commit_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    let mode = call
        .tools
        .service()
        .edit_mode(
            call.who,
            EditModeRequest {
                document: &call.document,
                requested: call.mode,
                actor: ActorKind::Agent,
            },
            call.ctx.trace_id(),
        )
        .await
        .map_err(tool_error)?;
    if mode == EditMode::Propose {
        let asked = call.mode;
        let (output, summary) = propose_ops(call).await?;
        if asked == Some(EditMode::Propose) {
            return Ok((output, summary));
        }
        return Ok((output, format!("{summary}. {CONVERTED}")));
    }
    let refs = image_refs(None, &call.ops);
    let media = resolve_media(call.tools, call.ctx, refs, Some(&call.document)).await?;
    let outcome = call
        .tools
        .service()
        .commit(
            call.who,
            CommitRequest {
                document_id: call.document.clone(),
                base_revision: call.base.clone(),
                body: CommitBody::Ops(call.ops.clone()),
                author: agent_author(call.ctx, call.exec_id),
                message: call.message.clone(),
                lock_holder_id: None,
                comments: call.comments.clone(),
                op_id: call.op_id.clone(),
                media,
            },
            call.ctx.trace_id(),
        )
        .await
        .map_err(tool_error)?;
    let landed = match outcome {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => *landed,
        CommitOutcome::Conflict { head, base } => {
            return Err(stale_base(&call.document, &head, base.as_ref()));
        },
    };
    let output = edit_ack(&call.document, landed, call.verbose);
    if !output.replayed {
        announce_agent(&call, &output).await;
    }
    let summary = if output.replayed {
        format!(
            "op {} replayed",
            call.op_id.as_ref().map_or("", |op| op.as_str())
        )
    } else {
        format!("wrote {}", output.written.len())
    };
    Ok((output, summary))
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
                    author: agent_author(ctx, exec_id),
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

#[derive(Debug, Clone, Copy)]
pub struct EditHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for EditHandler<'_> {
    type Input = EditInput;
    type Output = EditOutput;

    fn tool_name(&self) -> &'static str {
        "documents_edit"
    }

    fn description(&self) -> &'static str {
        "Apply ops, or propose them. Guide: editing."
    }

    async fn handle(
        &self,
        input: EditInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(EditOutput, String), McpError> {
        let who = caller(ctx)?;
        commit_ops(EditCall {
            tools: self.tools,
            who,
            ctx,
            exec_id,
            document: input.document_id,
            base: input.base_revision,
            ops: input.ops,
            mode: input.mode,
            op_id: input.op_id,
            message: input.message,
            comments: CommentPolicy {
                allow_detach: input.allow_detach,
                answering: input.answering,
            },
            verbose: input.verbose,
        })
        .await
    }
}
