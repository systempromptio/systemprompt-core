//! Reading reports: the caller's own, most recently seen first, and one
//! report with its triage state and attachments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_files::issues::IssueListQuery;
use systemprompt_identifiers::{IssueReportId, McpExecutionId};
use systemprompt_mcp::{McpOutputSchema, McpToolHandler};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::issues::{IssueReport, IssueStatus};

use super::IssuesMcpTools;
use super::refusal::{caller, tool_error};

const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
    #[serde(default)]
    pub status: Option<IssueStatus>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ListOutput {
    pub reports: Vec<IssueReport>,
}

impl McpOutputSchema for ListOutput {
    fn artifact_type() -> &'static str {
        "issues_list"
    }

    fn terse_wire(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListHandler<'a> {
    pub tools: &'a IssuesMcpTools,
}

impl McpToolHandler for ListHandler<'_> {
    type Input = ListInput;
    type Output = ListOutput;

    fn tool_name(&self) -> &'static str {
        "issues_list"
    }

    fn description(&self) -> &'static str {
        "Your user's issue reports, most recently seen first."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ListInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ListOutput, String), McpError> {
        let user = caller(ctx)?;
        let query = IssueListQuery {
            reporter: Some(user.id.clone()),
            status: input.status,
            before: None,
            limit: input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
        };
        let reports = self.tools.issues().list(&query).await.map_err(tool_error)?;
        let summary = format!("{} issue reports", reports.len());
        Ok((ListOutput { reports }, summary))
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusInput {
    pub report_id: IssueReportId,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StatusOutput {
    pub report: IssueReport,
}

impl McpOutputSchema for StatusOutput {
    fn artifact_type() -> &'static str {
        "issue_status"
    }

    fn terse_wire(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatusHandler<'a> {
    pub tools: &'a IssuesMcpTools,
}

impl McpToolHandler for StatusHandler<'_> {
    type Input = StatusInput;
    type Output = StatusOutput;

    fn tool_name(&self) -> &'static str {
        "issue_status"
    }

    fn description(&self) -> &'static str {
        "One of your issue reports: status, triage note, occurrences and attachments."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: StatusInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(StatusOutput, String), McpError> {
        let user = caller(ctx)?;
        let report = self
            .tools
            .issues()
            .find_visible(&input.report_id, &user.id, user.roles())
            .await
            .map_err(tool_error)?;
        let summary = format!("issue {} is {}", report.id, report.status);
        Ok((StatusOutput { report }, summary))
    }
}
