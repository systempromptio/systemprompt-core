use std::collections::{BTreeMap, HashMap};

use ego_tree::{NodeId, NodeRef};
use scraper::{ElementRef, Html, Node};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, NodeKind, ProjectionNode, hash8};

#[derive(Debug, Clone, Copy)]
pub(super) struct Located {
    pub tree_id: NodeId,
    pub kind: NodeKind,
}

#[derive(Debug)]
pub(super) struct HtmlProjection {
    pub root: ProjectionNode,
    pub handles: HashMap<DocumentNodeId, Located>,
    pub body: NodeId,
}

const SKIPPED: &[&str] = &["script", "style", "template", "noscript"];

fn block_kind(tag: &str) -> Option<NodeKind> {
    match tag {
        "h1" => Some(NodeKind::Heading { level: 1 }),
        "h2" => Some(NodeKind::Heading { level: 2 }),
        "h3" => Some(NodeKind::Heading { level: 3 }),
        "h4" => Some(NodeKind::Heading { level: 4 }),
        "h5" => Some(NodeKind::Heading { level: 5 }),
        "h6" => Some(NodeKind::Heading { level: 6 }),
        "p" => Some(NodeKind::Paragraph),
        "li" => Some(NodeKind::ListItem),
        "table" => Some(NodeKind::Table),
        "tr" => Some(NodeKind::Row),
        "td" | "th" => Some(NodeKind::Cell),
        "img" => Some(NodeKind::Image),
        _ => None,
    }
}

pub(super) fn find_body(html: &Html) -> Result<NodeId, CodecError> {
    html.tree
        .root()
        .descendants()
        .find(|node| matches!(node.value(), Node::Element(element) if element.name() == "body"))
        .map(|node| node.id())
        .ok_or(CodecError::Malformed {
            detail: "the document has no body element",
        })
}

pub(super) fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn node_text(node: NodeRef<'_, Node>, kind: NodeKind) -> Option<String> {
    match kind {
        NodeKind::Table | NodeKind::Row => None,
        NodeKind::Image => ElementRef::wrap(node)
            .and_then(|element| element.value().attr("alt"))
            .map(collapse),
        _ => ElementRef::wrap(node).map(|element| collapse(&element.text().collect::<String>())),
    }
}

pub(super) fn anchor_node(
    id: &DocumentNodeId,
    kind: NodeKind,
    text: Option<String>,
) -> ProjectionNode {
    ProjectionNode {
        id: id.clone(),
        kind,
        hash8: hash8(text.as_deref().unwrap_or_default()),
        text,
        children: Vec::new(),
        attrs: BTreeMap::new(),
    }
}

struct Walk<'a> {
    counters: HashMap<&'a str, usize>,
    handles: HashMap<DocumentNodeId, Located>,
}

impl<'a> Walk<'a> {
    fn children(&mut self, node: NodeRef<'a, Node>, out: &mut Vec<ProjectionNode>) {
        for child in node.children() {
            let Node::Element(element) = child.value() else {
                continue;
            };
            let tag = element.name();
            if SKIPPED.contains(&tag) {
                continue;
            }
            let Some(kind) = block_kind(tag) else {
                self.children(child, out);
                continue;
            };
            let counter = self.counters.entry(tag).or_insert(0);
            *counter += 1;
            let id = DocumentNodeId::new(format!("body/{tag}[{counter}]"));
            self.handles.insert(
                id.clone(),
                Located {
                    tree_id: child.id(),
                    kind,
                },
            );
            let mut projected = anchor_node(&id, kind, node_text(child, kind));
            if kind == NodeKind::Image {
                for name in ["src", "alt"] {
                    if let Some(value) = element.attr(name) {
                        projected.attrs.insert(name.to_owned(), value.to_owned());
                    }
                }
            }
            self.children(child, &mut projected.children);
            out.push(projected);
        }
    }
}

pub(super) fn project_tree(html: &Html) -> Result<HtmlProjection, CodecError> {
    let body = find_body(html)?;
    let body_ref = html.tree.get(body).ok_or(CodecError::Malformed {
        detail: "the document has no body element",
    })?;
    let mut walk = Walk {
        counters: HashMap::new(),
        handles: HashMap::new(),
    };
    let mut children = Vec::new();
    walk.children(body_ref, &mut children);
    let body_text = ElementRef::wrap(body_ref)
        .map(|element| element.text().collect::<String>())
        .unwrap_or_default();
    let mut root = anchor_node(&DocumentNodeId::new("body"), NodeKind::Body, None);
    root.hash8 = hash8(&body_text);
    root.children = children;
    Ok(HtmlProjection {
        root,
        handles: walk.handles,
        body,
    })
}
