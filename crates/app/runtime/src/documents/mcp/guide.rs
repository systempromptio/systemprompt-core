//! Tool descriptions carry one line each; the working detail lives here so
//! the tool list stays small and an agent pays for a topic only when it
//! needs it.
//!
//! The `formats` topic is built from the codec registry, so the node kinds
//! it lists for `insert_after` are the ones each codec accepts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::guide_topics::{
    COMMENTS, EDITING, LIFECYCLE, PROPOSALS, SHARING, SHEETS, SLIDES, refusal_text,
};
use super::inputs::GuideInput;
use super::outputs::GuideOutput;
use super::refusal::RefusalCode;

fn formats(tools: &DocumentsMcpTools) -> String {
    let service = tools.service();
    let mut out =
        String::from("insert_after kinds per format (an empty list means no insert_after):");
    for format in service.formats() {
        let kinds = service.insert_kinds(format);
        out.push_str(&format!("\n{format}: {}", kinds.join(", ")));
    }
    out.push_str(
        "\nExport: native as url everywhere; html as preview everywhere; md or html as url \
         only for a document already in that format; pdf is unsupported_format.",
    );
    out
}

fn topic(tools: &DocumentsMcpTools, name: &str) -> String {
    let body = match name {
        "editing" => EDITING.to_owned(),
        "formats" => formats(tools),
        "sheets" => SHEETS.to_owned(),
        "slides" => SLIDES.to_owned(),
        "sharing" => SHARING.to_owned(),
        "comments" => COMMENTS.to_owned(),
        "lifecycle" => LIFECYCLE.to_owned(),
        "proposals" => PROPOSALS.to_owned(),
        other => RefusalCode::ALL
            .iter()
            .find(|code| other.strip_prefix("refusal.") == Some(code.as_str()))
            .map_or_else(
                || {
                    format!(
                        "unknown topic; topics are editing, formats, sheets, slides, sharing, \
                         comments, lifecycle, proposals and refusal.<code> for {}",
                        RefusalCode::ALL.map(RefusalCode::as_str).join(", ")
                    )
                },
                |code| refusal_text(*code).to_owned(),
            ),
    };
    format!("## {name}\n{body}")
}

#[derive(Debug, Clone, Copy)]
pub struct GuideHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for GuideHandler<'_> {
    type Input = GuideInput;
    type Output = GuideOutput;

    fn tool_name(&self) -> &'static str {
        "documents_guide"
    }

    fn description(&self) -> &'static str {
        "Detail by topic (editing, formats, proposals, refusal.<code>, ...)."
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
        let text = input
            .topics
            .iter()
            .map(|name| topic(self.tools, name))
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok((
            GuideOutput {
                topics: input.topics,
            },
            text,
        ))
    }
}
