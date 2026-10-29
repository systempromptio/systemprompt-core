//! Filing records the call's own trace, context, session, task and MCP
//! execution rather than anything the agent pastes, so a report points at
//! what happened and carries no copied log line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_files::issues::IssueFiling;
use systemprompt_identifiers::{AgentName, IssueReportId, McpExecutionId};
use systemprompt_mcp::{McpOutputSchema, McpToolHandler};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity};

use super::IssuesMcpTools;
use super::refusal::{caller, tool_error};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportInput {
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    #[schemars(description = "One line: what went wrong, at most 200 characters.")]
    pub title: String,
    #[schemars(
        description = "What you did, what you expected, what happened. No logs or secrets."
    )]
    #[serde(default)]
    pub body: String,
    #[schemars(
        description = "A stable key for this problem (e.g. tool name and error code); repeats with \
                       the same key fold into one open report. Defaults to the title."
    )]
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[schemars(description = "Client facts: product and version, OS. A small JSON object.")]
    #[serde(default)]
    pub client: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReportOutput {
    pub report_id: IssueReportId,
    pub folded: bool,
    pub occurrences: i32,
}

impl McpOutputSchema for ReportOutput {
    fn artifact_type() -> &'static str {
        "report_issue"
    }

    fn terse_wire(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReportHandler<'a> {
    pub tools: &'a IssuesMcpTools,
}

pub(super) fn issue_context(ctx: &RequestContext, exec_id: &McpExecutionId) -> IssueContext {
    let present = |value: &str| !value.is_empty();
    IssueContext {
        trace: Some(ctx.trace_id().clone()).filter(|id| present(id.as_str())),
        context: Some(ctx.context_id().clone()).filter(|id| present(id.as_str())),
        session: Some(ctx.session_id().clone()).filter(|id| present(id.as_str())),
        task: ctx.task_id().cloned(),
        mcp_execution: Some(exec_id.clone()),
    }
}

impl McpToolHandler for ReportHandler<'_> {
    type Input = ReportInput;
    type Output = ReportOutput;

    fn tool_name(&self) -> &'static str {
        "report_issue"
    }

    fn description(&self) -> &'static str {
        "File a bug or feedback report once per problem; repeats fold into the open report."
    }

    async fn handle(
        &self,
        input: ReportInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(ReportOutput, String), McpError> {
        let user = caller(ctx)?;
        let agent = ctx.agent_name();
        let filing = IssueFiling {
            reporter: user.id.clone(),
            via_agent: (!agent.as_str().is_empty() && *agent != AgentName::system())
                .then(|| agent.as_str().chars().take(64).collect()),
            kind: input.kind,
            severity: input.severity,
            title: input.title,
            body: input.body,
            fingerprint_key: input.fingerprint,
            context: issue_context(ctx, exec_id),
            client: input.client.unwrap_or(serde_json::Value::Null),
        };
        let (report_id, filed) = self.tools.issues().file(filing).await.map_err(tool_error)?;
        let summary = if filed.inserted {
            format!("filed issue {report_id}")
        } else {
            format!(
                "folded into open issue {report_id} ({} occurrences)",
                filed.occurrences
            )
        };
        Ok((
            ReportOutput {
                report_id,
                folded: !filed.inserted,
                occurrences: filed.occurrences,
            },
            summary,
        ))
    }
}
