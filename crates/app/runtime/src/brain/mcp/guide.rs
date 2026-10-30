//! Tool descriptions carry one line each; the working detail lives here by
//! topic, so the tool list stays small.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::refusal::BrainRefusalCode;
use super::tools::{GuideInput, GuideOutput};

const TOPICS: [&str; 5] = ["facts", "scopes", "recall", "proposing", "refusals"];

const FACTS: &str = "A fact is one revisioned statement about a subject (`kind:id`) of a \
declared kind, with its source, status and scope. Only confirmed facts are recalled. A \
notification is different: it tells a person something happened (a fact was proposed, \
disputed or is due for review) and is never memory. brain_search lists facts you may see; \
brain_why explains one fact's score.";

const SCOPES: &str = "A fact is scoped to a user (only its owner sees it), a project, a group \
or the org. A proposal defaults to your user's own scope; pass scope {level, key} to aim \
wider. A project or wider fact is confirmed by an admin or a workflow's confirm role.";

const RECALL: &str = "Memory is attached automatically: the gateway injects a <team_memory> \
block of confirmed facts on each new user turn, within a token budget. You do not fetch \
memory to use it. These tools are for inspection and proposing. brain_recall_preview shows \
what would be injected for a prompt and why facts were withheld; it records nothing.";

const PROPOSING: &str = "brain_propose enters a fact as proposed: {kind, subject: \
\"kind:id\", fields, topics?, scope?}. A person confirms or rejects it. An agent never \
confirms, retracts, pins or erases a fact; asking to is refused with agent_forbidden. \
brain_dispute {fact_id, reason} flags a fact you have reason to doubt.";

#[must_use]
pub const fn refusal_text(code: BrainRefusalCode) -> &'static str {
    match code {
        BrainRefusalCode::BrainDisabled => "brain_disabled: the brain is off for this deployment.",
        BrainRefusalCode::UnknownKind => {
            "unknown_kind: the kind is not declared; use a kind from brain_search results."
        },
        BrainRefusalCode::Forbidden => {
            "forbidden: the fact or scope is not yours to see or enter into."
        },
        BrainRefusalCode::AgentForbidden => {
            "agent_forbidden: agents propose and dispute; a person confirms."
        },
        BrainRefusalCode::NotFound => "not_found: no fact has that id.",
        BrainRefusalCode::Tombstoned => {
            "tombstoned: that fact was erased and may not be recorded again."
        },
        BrainRefusalCode::InvalidInput => {
            "invalid_input: an argument is malformed or an unknown key was sent."
        },
    }
}

fn refusals() -> String {
    BrainRefusalCode::ALL.map(refusal_text).join("\n")
}

fn topic(name: &str) -> String {
    let body = match name {
        "facts" => FACTS.to_owned(),
        "scopes" => SCOPES.to_owned(),
        "recall" => RECALL.to_owned(),
        "proposing" => PROPOSING.to_owned(),
        "refusals" => refusals(),
        _ => format!("unknown topic; topics are {}", TOPICS.join(", ")),
    };
    format!("## {name}\n{body}")
}

#[must_use]
pub fn guide_text(topics: &[String]) -> String {
    if topics.is_empty() {
        return TOPICS.map(topic).join("\n\n");
    }
    topics
        .iter()
        .map(|name| topic(name))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Debug, Clone, Copy)]
pub struct GuideHandler;

impl McpToolHandler for GuideHandler {
    type Input = GuideInput;
    type Output = GuideOutput;

    fn tool_name(&self) -> &'static str {
        "brain_guide"
    }

    fn description(&self) -> &'static str {
        "Detail by topic (facts, scopes, recall, proposing, refusals)."
    }

    fn read_only(&self) -> bool {
        true
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the handler trait is async; the guide is assembled from static text"
    )]
    async fn handle(
        &self,
        input: GuideInput,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(GuideOutput, String), McpError> {
        let guide = guide_text(&input.topics);
        Ok((
            GuideOutput {
                topics: input.topics,
                guide: guide.clone(),
            },
            guide,
        ))
    }
}
