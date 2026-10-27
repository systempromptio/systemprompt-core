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
use super::inputs::GuideInput;
use super::outputs::GuideOutput;
use super::refusal::RefusalCode;

const EDITING: &str = "\
Read: documents_read view outline first; each line is `<node>:<hash>| <text>`, with `# ` per \
heading level, `- ` for a list item, `| ` for a table cell and `## slide N` at each slide. The \
prefix is display only.
Edit: documents_edit {document_id, base_revision: <read revision_id>, ops, message}. Ops: \
replace_text {node, expected_hash, text}; delete {node, expected_hash}; insert_after {node, \
kind: {type: heading, level} | {type: paragraph} | {type: list_item} | {type: row} | {type: \
text}, text}; str_replace {old_str, new_str} (old_str must occur once); set_cells {sheet, \
range, values}; insert_slide {after, layout, title, body}; set_text {text}; set_html {html}.
Batch: every target resolves against the document as it stood before the call; ops apply in \
order, so two insert_after on one node land in op order; all ops apply or none.
Ack: an edit or create returns `written`, the lines it changed or created; anchor the next \
edit on them and use the returned revision_id as base_revision. Do not re-read between your \
own writes.
Conflict: a stale_base or guard_mismatch means someone else edited first. Their edit wins: \
re-aim from the returned text and never resend your old text over theirs.
Terse output: verbose: true on read, edit, history or list adds URIs and content hashes.";

const SHEETS: &str = "\
Cells read as `<sheet>!<cell>:<hash>| <value>` with view cells, sheet and an A1 range such as \
A1:C9. set_cells {sheet, range, values} writes a rectangle: values is rows of cells, each a \
number, bool, string or null, and its shape must match the range; a string starting with = is \
a formula. Rows are sheet[<name>]/row[N]: insert_after on a row adds a row below it from \
tab-separated text; replace_text rewrites a row or a cell; delete clears it.";

const SLIDES: &str = "\
A slide is slide[N]; its paragraphs are slide[N]/shape[H]/p[P]. insert_slide {after: \
slide[N], layout, title, body: [lines]} adds a slide; delete on slide[N] removes it. \
insert_after on a paragraph takes kind paragraph or list_item. Slide numbers in one call all \
name the deck as read.";

const SHARING: &str = "\
documents_share {document_id | folder_id, subject_kind, subject_value, level, valid_until}. \
subject_kind is user, role or a registered subject dimension such as group or project; level \
is view, comment, edit, share or own. A folder grant covers its subtree. Sharing needs share \
access.";

const fn refusal_text(code: RefusalCode) -> &'static str {
    match code {
        RefusalCode::StaleBase => {
            "stale_base: the document moved past your base_revision; head_revision, written_by, \
             actor_kind and at say who. Their edit wins: read head_revision, re-aim at the text \
             as it stands, never resend your old text."
        },
        RefusalCode::GuardMismatch => {
            "guard_mismatch: the node changed since you read it; current_text and hash are its \
             state now. Re-aim at current_text with hash as expected_hash, or drop the edit."
        },
        RefusalCode::FindNone => {
            "find_none: old_str matched `matches` places, not one. Quote it exactly as read and \
             widen it until it is unique."
        },
        RefusalCode::UnsupportedOp => {
            "unsupported_op: this format does not take the op or node kind; accepted_kinds lists \
             what insert_after takes. See topic formats."
        },
        RefusalCode::UnsupportedFormat => {
            "unsupported_format: no codec handles the requested format; export native as url, \
             or html as preview."
        },
        RefusalCode::TemplateFailed => {
            "template_failed: the template could not be instantiated; the text names the \
             template and the cause. Pick another template or report it."
        },
        RefusalCode::Forbidden => {
            "forbidden: you lack the access level the action needs; the text names the level \
             held and needed. Ask the owner to share."
        },
        RefusalCode::NotFound => {
            "not_found: the document, folder, revision or node does not exist or is deleted; \
             read the outline again for node ids."
        },
        RefusalCode::Locked => "locked: another holder has the document locked; wait and retry.",
        RefusalCode::InvalidParams => {
            "invalid_params: the call is malformed; fix the named field and resend."
        },
    }
}

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
        other => RefusalCode::ALL
            .iter()
            .find(|code| other.strip_prefix("refusal.") == Some(code.as_str()))
            .map_or_else(
                || {
                    format!(
                        "unknown topic; topics are editing, formats, sheets, slides, sharing and \
                         refusal.<code> for {}",
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
        "Working detail by topic: editing, formats, sheets, slides, sharing, refusal.<code>."
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
