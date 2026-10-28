//! The `documents_people` tool finds people a caller can mention or share with.
//!
//! It searches active people by name, email or id prefix through the store's
//! subject search (the one directory authority). With `document_id`, each match
//! carries the level it already holds there, so an agent knows who a mention
//! would share the document with; reading that needs comment on the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{McpExecutionId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::PeopleInput;
use super::outputs::{PeopleOutput, PersonItem};
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::refusal::RefusalCode;

const DEFAULT_PEOPLE: i64 = 10;

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
        "Find people to mention or share with; with document_id, their access."
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
        if input.query.trim().is_empty() {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "query needs a name, email or id",
            ));
        }
        let limit = usize::try_from(input.limit.unwrap_or(DEFAULT_PEOPLE)).unwrap_or(1);
        let service = self.tools.service();
        let found = service
            .access()
            .search_subjects("user", &input.query, limit)
            .await
            .map_err(tool_error)?;
        let users: Vec<UserId> = found
            .matches
            .iter()
            .map(|subject| UserId::new(subject.value.clone()))
            .collect();
        let levels = match &input.document_id {
            Some(document) => service
                .levels_on(who, document, &users, ctx.trace_id())
                .await
                .map_err(tool_error)?,
            None => vec![None; users.len()],
        };
        let people: Vec<PersonItem> = found
            .matches
            .into_iter()
            .zip(users)
            .zip(levels)
            .map(|((subject, user_id), access)| PersonItem {
                user_id,
                name: subject.display,
                email: subject.detail,
                access,
            })
            .collect();
        let summary = format!("{} people", people.len());
        Ok((PeopleOutput { people }, summary))
    }
}
