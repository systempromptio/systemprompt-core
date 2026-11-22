//! The `documents_people` tool finds people a caller can mention or share with.
//!
//! It searches active people whose name, email or id has a word starting with
//! each query word, accents ignored, through
//! [`people_search_view`], the view the HTTP route answers with too. With
//! `document_id`, each match carries the level it already holds there, so an
//! agent knows who a mention would share the document with; reading that
//! needs comment on the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::PeopleInput;
use super::support::{caller, tool_error};
use crate::documents::views::{DEFAULT_PEOPLE, PeopleOutput, PeopleSearch, people_search_view};

#[derive(Debug, Clone, Copy)]
pub struct PeopleHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for PeopleHandler<'_> {
    type Input = PeopleInput;
    type Output = PeopleOutput;

    fn tool_name(&self) -> &'static str {
        "documents_people"
    }

    fn description(&self) -> &'static str {
        "Find people to mention or share. Each query word must start a word of their name, email or id, in any order; accents are ignored."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: PeopleInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(PeopleOutput, String), McpError> {
        let who = caller(ctx)?;
        let limit = input
            .limit
            .map_or(Ok(DEFAULT_PEOPLE), usize::try_from)
            .unwrap_or(1);
        let search = PeopleSearch {
            query: &input.query,
            limit,
            document: input.document_id.as_ref(),
        };
        let found = people_search_view(self.tools.service(), who, search, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!("{} people", found.people.len());
        Ok((found, summary))
    }
}
