//! Semantic HTML preview of the document body, sanitised with `ammonia`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::NodeKind;

use super::projection::para_kind;
use super::scan::{Block, Para, RunText};

pub(super) fn render(blocks: &[Block]) -> String {
    let mut html = String::new();
    render_blocks(blocks, &mut html);
    ammonia::clean(&html)
}

fn render_blocks(blocks: &[Block], html: &mut String) {
    let mut in_list = false;
    for block in blocks {
        let list_item = matches!(block, Block::Para(p) if para_kind(p) == NodeKind::ListItem);
        if list_item && !in_list {
            html.push_str("<ul>");
        } else if !list_item && in_list {
            html.push_str("</ul>");
        }
        in_list = list_item;
        match block {
            Block::Para(para) => render_para(para, html),
            Block::Table(table) => {
                html.push_str("<table>");
                for row in &table.rows {
                    html.push_str("<tr>");
                    for cell in row {
                        html.push_str("<td>");
                        render_blocks(&cell.blocks, html);
                        html.push_str("</td>");
                    }
                    html.push_str("</tr>");
                }
                html.push_str("</table>");
            },
        }
    }
    if in_list {
        html.push_str("</ul>");
    }
}

fn render_para(para: &Para, html: &mut String) {
    let tag = match para_kind(para) {
        NodeKind::Heading { level } => format!("h{}", level.min(6)),
        NodeKind::ListItem => "li".to_owned(),
        _ => "p".to_owned(),
    };
    html.push_str(&format!("<{tag}>"));
    for run in &para.runs {
        render_run(run, html);
    }
    html.push_str(&format!("</{tag}>"));
}

fn render_run(run: &RunText, html: &mut String) {
    let wrappers: Vec<&str> = [
        (run.bold, "strong"),
        (run.italic, "em"),
        (run.underline, "u"),
    ]
    .into_iter()
    .filter_map(|(on, tag)| on.then_some(tag))
    .collect();
    for tag in &wrappers {
        html.push_str(&format!("<{tag}>"));
    }
    html.push_str(&escape(run.text.as_str()).replace('\n', "<br>"));
    for tag in wrappers.iter().rev() {
        html.push_str(&format!("</{tag}>"));
    }
}
