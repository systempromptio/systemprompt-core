//! The HTML codec's edit operations, applied to the parsed DOM.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use ego_tree::NodeId;
use scraper::node::Text;
use scraper::{ElementRef, Html, Node, StrTendril};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, required_hash,
};

use super::super::anchors::{str_replace_once, verify_anchor};
use super::super::text::escape_html;
use super::markup::{markdown_markup, pending_markup};
use super::tree::{HtmlProjection, Located, anchor_node, node_text};

const DETACHED: CodecError = CodecError::Malformed {
    detail: "an edited node is no longer part of the document",
};

const fn unsupported(op: &EditOp) -> CodecError {
    CodecError::UnsupportedOp {
        op: op.name(),
        format: DocumentFormat::Html,
    }
}

fn is_attached(html: &Html, id: NodeId) -> bool {
    let root = html.tree.root().id();
    html.tree
        .get(id)
        .is_some_and(|node| node.ancestors().any(|ancestor| ancestor.id() == root))
}

fn locate(
    html: &Html,
    projection: &HtmlProjection,
    node: &DocumentNodeId,
) -> Result<Located, CodecError> {
    projection
        .handles
        .get(node)
        .copied()
        .filter(|located| is_attached(html, located.tree_id))
        .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })
}

fn verify(
    html: &Html,
    node: &DocumentNodeId,
    located: Located,
    expected: &str,
) -> Result<(), CodecError> {
    let tree_node = html.tree.get(located.tree_id).ok_or(DETACHED)?;
    verify_anchor(
        &anchor_node(node, located.kind, node_text(tree_node, located.kind)),
        expected,
    )
}

fn clear_children(html: &mut Html, id: NodeId) -> Result<(), CodecError> {
    let mut target = html.tree.get_mut(id).ok_or(DETACHED)?;
    while let Some(mut child) = target.first_child() {
        child.detach();
    }
    Ok(())
}

fn parse_into(html: &mut Html, fragment: &str) -> Result<Vec<NodeId>, CodecError> {
    let parsed = Html::parse_fragment(fragment);
    let orphan = html.tree.extend_tree(parsed.tree).id();
    let container = html
        .tree
        .get(orphan)
        .and_then(|root| root.children().find(|child| child.value().is_element()))
        .ok_or(CodecError::Malformed {
            detail: "the inserted markup did not parse",
        })?;
    Ok(container.children().map(|child| child.id()).collect())
}

fn set_body(html: &mut Html, body: NodeId, markup: &str) -> Result<(), CodecError> {
    clear_children(html, body)?;
    let nodes = parse_into(html, &ammonia::clean(markup))?;
    let mut target = html.tree.get_mut(body).ok_or(DETACHED)?;
    for id in nodes {
        target.append_id(id);
    }
    Ok(())
}

fn body_inner(html: &Html, body: NodeId) -> Result<String, CodecError> {
    html.tree
        .get(body)
        .and_then(ElementRef::wrap)
        .map(|element| element.inner_html())
        .ok_or(DETACHED)
}

fn insert_tag(kind: NodeKind) -> Option<String> {
    match kind {
        NodeKind::Paragraph => Some("p".to_owned()),
        NodeKind::ListItem => Some("li".to_owned()),
        NodeKind::Heading { level } => Some(format!("h{}", level.clamp(1, 6))),
        _ => None,
    }
}

fn insert_after(
    html: &mut Html,
    projection: &HtmlProjection,
    op: &EditOp,
    tails: &mut HashMap<NodeId, NodeId>,
) -> Result<(), CodecError> {
    let EditOp::InsertAfter {
        node,
        kind,
        text,
        content_as,
    } = op
    else {
        return Err(unsupported(op));
    };
    let node = node.resolved()?;
    let markup = match content_as {
        ContentAs::Markdown => markdown_markup(text),
        ContentAs::Text if *kind == NodeKind::Pending => pending_markup(text),
        ContentAs::Text => {
            let tag = insert_tag(*kind).ok_or_else(|| unsupported(op))?;
            format!("<{tag}>{}</{tag}>", escape_html(text))
        },
    };
    let anchor = if node.as_str() == "body" {
        None
    } else {
        Some(locate(html, projection, node)?.tree_id)
    };
    let key = anchor.unwrap_or(projection.body);
    let nodes = parse_into(html, &markup)?;
    let Some(&last) = nodes.last() else {
        return Ok(());
    };
    if let Some(after) = tails.get(&key).copied().or(anchor) {
        let mut at = html.tree.get_mut(after).ok_or(DETACHED)?;
        for id in nodes.into_iter().rev() {
            at.insert_id_after(id);
        }
    } else {
        let mut body = html.tree.get_mut(projection.body).ok_or(DETACHED)?;
        for id in nodes.into_iter().rev() {
            body.prepend_id(id);
        }
    }
    tails.insert(key, last);
    Ok(())
}

fn fill_pending(html: &mut Html, pending: NodeId) -> Result<NodeId, CodecError> {
    let nodes = parse_into(html, "<p></p>")?;
    let filled = *nodes.first().ok_or(DETACHED)?;
    let mut at = html.tree.get_mut(pending).ok_or(DETACHED)?;
    at.insert_id_after(filled);
    at.detach();
    Ok(filled)
}

fn append_markdown(html: &mut Html, body: NodeId, markdown: &str) -> Result<(), CodecError> {
    let nodes = parse_into(html, &markdown_markup(markdown))?;
    let mut target = html.tree.get_mut(body).ok_or(DETACHED)?;
    for id in nodes {
        target.append_id(id);
    }
    Ok(())
}

fn replace_text(
    html: &mut Html,
    projection: &HtmlProjection,
    op: &EditOp,
    (node, expected_hash, text): (&NodeTarget, Option<&String>, &str),
) -> Result<(), CodecError> {
    let node = node.resolved()?;
    let located = locate(html, projection, node)?;
    if matches!(
        located.kind,
        NodeKind::Table | NodeKind::Row | NodeKind::Image
    ) {
        return Err(unsupported(op));
    }
    verify(html, node, located, required_hash(expected_hash)?)?;
    let target_id = if located.kind == NodeKind::Pending {
        fill_pending(html, located.tree_id)?
    } else {
        clear_children(html, located.tree_id)?;
        located.tree_id
    };
    let mut target = html.tree.get_mut(target_id).ok_or(DETACHED)?;
    target.append(Node::Text(Text {
        text: StrTendril::from_slice(text),
    }));
    Ok(())
}

pub(super) fn apply_ops(
    html: &mut Html,
    projection: &HtmlProjection,
    ops: &[EditOp],
) -> Result<(), CodecError> {
    ops.iter().try_for_each(EditOp::check_resolved)?;
    let mut rewritten = false;
    let mut tails = HashMap::new();
    for op in ops {
        if rewritten && op.touches().is_some() {
            return Err(CodecError::Malformed {
                detail: "an anchored op cannot follow a whole-document op in the same batch",
            });
        }
        match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                ..
            } => replace_text(html, projection, op, (node, expected_hash.as_ref(), text))?,
            EditOp::Delete {
                node,
                expected_hash,
            } => {
                let node = node.resolved()?;
                let located = locate(html, projection, node)?;
                verify(html, node, located, required_hash(expected_hash.as_ref())?)?;
                html.tree.get_mut(located.tree_id).ok_or(DETACHED)?.detach();
            },
            EditOp::InsertAfter { .. } => insert_after(html, projection, op, &mut tails)?,
            EditOp::StrReplace { old_str, new_str } => {
                let replaced =
                    str_replace_once(&body_inner(html, projection.body)?, old_str, new_str)?;
                set_body(html, projection.body, &replaced)?;
                rewritten = true;
            },
            EditOp::AppendMarkdown { markdown } => {
                append_markdown(html, projection.body, markdown)?;
            },
            EditOp::SetHtml { html: markup } => {
                set_body(html, projection.body, markup)?;
                rewritten = true;
            },
            EditOp::SetCells { .. }
            | EditOp::InsertSlide { .. }
            | EditOp::SetText { .. }
            | EditOp::Restore { .. }
            | EditOp::InsertFromRevision { .. }
            | EditOp::SetMarks(_)
            | EditOp::SetBlockType(_)
            | EditOp::InsertTable(_)
            | EditOp::TableInsertRow(_)
            | EditOp::TableInsertCol(_)
            | EditOp::TableDelete(_)
            | EditOp::InsertImage(_)

            | EditOp::SetParagraph(_)

            | EditOp::InsertBlock(_)

            | EditOp::UpdateTable(_)

            | EditOp::SetSection(_)

            | EditOp::SetStyle(_)

            | EditOp::ApplyTheme(_) => {
                return Err(unsupported(op));
            },
        }
    }
    Ok(())
}
