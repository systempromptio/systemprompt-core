use std::collections::{BTreeMap, HashSet};

use systemprompt_files::documents::nodes::{CarryHints, NodeIdMinter, carry_forward};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash,
};

fn node(id: &str, kind: NodeKind, text: Option<&str>, hash: &str) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text: text.map(str::to_owned),
        hash8: hash.to_owned(),
        children: Vec::new(),
        attrs: BTreeMap::new(),
    }
}

fn para(id: &str, text: &str, hash: &str) -> ProjectionNode {
    node(id, NodeKind::Paragraph, Some(text), hash)
}

fn projection(root: Vec<ProjectionNode>) -> Projection {
    Projection {
        format: DocumentFormat::Markdown,
        root,
        content_sha256: content_hash(b""),
    }
}

fn carried(old: Vec<ProjectionNode>, new: Vec<ProjectionNode>, rewritten: &[&str]) -> Vec<String> {
    let hints = CarryHints {
        retired: HashSet::new(),
        rewritten: rewritten
            .iter()
            .map(|id| DocumentNodeId::new(*id))
            .collect(),
    };
    carry_forward(
        &projection(old),
        &projection(new),
        &hints,
        &mut NodeIdMinter::new("m".to_owned(), 0),
    )
    .into_iter()
    .map(|entry| entry.node_id.as_str().to_owned())
    .collect()
}

fn anchored(
    middle_old: Vec<ProjectionNode>,
    middle_new: Vec<ProjectionNode>,
) -> [Vec<ProjectionNode>; 2] {
    let wrap = |middle: Vec<ProjectionNode>| {
        let mut all = vec![para("a", "start", "h-start")];
        all.extend(middle);
        all.push(para("z", "end", "h-end"));
        all
    };
    [wrap(middle_old), wrap(middle_new)]
}

#[test]
fn a_rewritten_paragraph_turned_heading_keeps_its_id_in_a_single_gap() {
    let [old, new] = anchored(
        vec![para("x", "old words", "h-x")],
        vec![node(
            "new",
            NodeKind::Heading { level: 1 },
            Some("other"),
            "h-y",
        )],
    );
    assert_eq!(
        carried(old.clone(), new.clone(), &["x"]),
        vec!["a", "x", "z"]
    );
    assert_eq!(carried(old, new, &[]), vec!["a", "m.1", "z"]);
}

#[test]
fn a_single_gap_of_different_non_text_kinds_is_minted_even_when_rewritten() {
    let [old, new] = anchored(
        vec![node("img", NodeKind::Image, None, "h-img")],
        vec![node("t", NodeKind::Table, None, "h-t")],
    );
    assert_eq!(carried(old, new, &["img"]), vec!["a", "m.1", "z"]);
}

#[test]
fn in_a_wide_gap_blank_texts_pair_and_a_lost_text_is_minted() {
    let [old, new] = anchored(
        vec![para("blank", "   ", "h-b1"), para("lost", "words", "h-l1")],
        vec![
            para("blank2", "", "h-b2"),
            node("textless", NodeKind::Paragraph, None, "h-l2"),
        ],
    );
    assert_eq!(carried(old, new, &[]), vec!["a", "blank", "m.1", "z"]);
}

#[test]
fn in_a_wide_gap_a_rewritten_text_block_of_another_kind_keeps_its_id() {
    let [old, new] = anchored(
        vec![para("p1", "alpha", "h-1"), para("p2", "beta", "h-2")],
        vec![
            node("h", NodeKind::Heading { level: 2 }, Some("totally"), "h-3"),
            node("l", NodeKind::ListItem, Some("different"), "h-4"),
        ],
    );
    assert_eq!(
        carried(old.clone(), new.clone(), &["p1"]),
        vec!["a", "p1", "m.1", "z"]
    );
    assert_eq!(carried(old, new, &[]), vec!["a", "m.1", "m.2", "z"]);
}

#[test]
fn similar_text_in_a_wide_gap_keeps_its_id_from_the_back() {
    let [old, new] = anchored(
        vec![
            para("p1", "one", "h-1"),
            para("p2", "the quick brown fox", "h-2"),
        ],
        vec![
            node("img", NodeKind::Image, None, "h-3"),
            para("p", "the quick brown fox jumps", "h-4"),
        ],
    );
    assert_eq!(carried(old, new, &[]), vec!["a", "m.1", "p2", "z"]);
}

#[test]
fn a_very_large_reorder_aligns_greedily_and_keeps_ids() {
    let count = 2_001;
    let old: Vec<ProjectionNode> = (0..count)
        .map(|i| para(&format!("o{i}"), &format!("text {i}"), &format!("h{i}")))
        .collect();
    let mut new: Vec<ProjectionNode> = (0..count)
        .map(|i| para(&format!("n{i}"), &format!("text {i}"), &format!("h{i}")))
        .collect();
    new[0] = para("first", "replaced first", "h-first");
    new[count - 1] = para("last", "replaced last", "h-last");
    new.insert(1_000, node("inserted", NodeKind::Image, None, "h-new"));
    let ids = carried(old, new, &[]);
    assert_eq!(ids.len(), count + 1);
    assert_eq!(ids[0], "o0");
    assert_eq!(ids[999], "o999");
    assert_eq!(ids[1_000], "m.1");
    assert_eq!(ids[1_001], "o1000");
    assert_eq!(ids[count], format!("o{}", count - 1));
    assert_eq!(ids.iter().filter(|id| id.starts_with("m.")).count(), 1);
}
