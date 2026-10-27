use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode, content_hash, hash8};

fn node(id: &str, text: &str, children: Vec<ProjectionNode>) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind: NodeKind::Paragraph,
        text: Some(text.to_owned()),
        hash8: hash8(text),
        children,
        attrs: BTreeMap::new(),
    }
}

fn sample() -> Projection {
    Projection {
        format: systemprompt_models::documents::DocumentFormat::Markdown,
        root: vec![
            node(
                "h1",
                "Title",
                vec![node("p1", "one", vec![]), node("p2", "two", vec![])],
            ),
            node("p3", "three", vec![]),
        ],
        content_sha256: content_hash(b"doc"),
    }
}

#[test]
fn hash8_is_eight_lowercase_hex() {
    let h = hash8("hello world");
    assert_eq!(h.len(), 8);
    assert!(
        h.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(h, "b94d27b9");
}

#[test]
fn hash8_normalises_whitespace() {
    assert_eq!(hash8("hello world"), hash8("  hello \n\t  world  "));
    assert_ne!(hash8("hello world"), hash8("helloworld"));
}

#[test]
fn content_hash_is_sha256_hex() {
    assert_eq!(
        content_hash(b"test").as_str(),
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    );
}

#[test]
fn hashline_renders_id_hash_and_text() {
    let n = node("p1", "one", vec![]);
    assert_eq!(n.hashline(), format!("p1:{}| one", hash8("one")));
}

#[test]
fn find_walks_children() {
    let p = sample();
    assert_eq!(
        p.find(&DocumentNodeId::new("p2"))
            .and_then(|n| n.text.as_deref()),
        Some("two")
    );
    assert!(p.find(&DocumentNodeId::new("missing")).is_none());
}

#[test]
fn lines_are_depth_first() {
    let ids: Vec<String> = sample()
        .lines(None)
        .into_iter()
        .map(|l| l.split(':').next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(ids, ["h1", "p1", "p2", "p3"]);
}

#[test]
fn lines_honour_inclusive_range() {
    let p = sample();
    let start = DocumentNodeId::new("p1");
    let end = DocumentNodeId::new("p3");
    let lines = p.lines(Some((&start, &end)));
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("p1:"));
    assert!(lines[2].starts_with("p3:"));
}

#[test]
fn projection_serde_round_trip() {
    let p = sample();
    let json = serde_json::to_string(&p).expect("serialize");
    let back: Projection = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, p);
}
