use std::num::NonZeroUsize;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::{MarkdownCodec, TextCodec};
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, QuoteTarget, TargetError,
    resolve_targets,
};

use super::{SAMPLE_MD, id};

const LINES: &[u8] = b"alpha one\nbeta two\nalpha three\n";

fn quote(find: &str, nth: Option<usize>, within: Option<&str>) -> NodeTarget {
    NodeTarget::Find(QuoteTarget {
        find: find.to_owned(),
        nth: nth.and_then(NonZeroUsize::new),
        within: within.map(id),
    })
}

fn delete(target: NodeTarget) -> EditOp {
    EditOp::Delete {
        node: target,
        expected_hash: None,
    }
}

fn resolve_and_apply(codec: &dyn DocumentCodec, bytes: &[u8], ops: Vec<EditOp>) -> Vec<u8> {
    let base = codec.project(bytes).expect("project");
    let resolved = resolve_targets(&base, ops).expect("resolve");
    codec.apply(bytes, &base, &resolved.ops).expect("apply")
}

fn resolved_node(op: &EditOp) -> &str {
    match op {
        EditOp::ReplaceText { node, .. }
        | EditOp::InsertAfter { node, .. }
        | EditOp::Delete { node, .. } => node.id().map(|n| n.as_str()).expect("resolved id"),
        other => panic!("not anchored: {other:?}"),
    }
}

#[test]
fn nth_picks_among_matches_and_reports_the_count() {
    let base = TextCodec.project(LINES).expect("project");
    let resolved =
        resolve_targets(&base, vec![delete(quote("alpha", Some(2), None))]).expect("resolve");
    assert_eq!(resolved_node(&resolved.ops[0]), "line[3]");
    assert_eq!(resolved.matches, vec![2]);
    let EditOp::Delete { expected_hash, .. } = &resolved.ops[0] else {
        panic!("delete");
    };
    assert_eq!(
        expected_hash.as_deref(),
        Some(super::hash_of(&base, "line[3]").as_str())
    );
}

#[test]
fn without_nth_the_first_match_is_used() {
    let out = resolve_and_apply(&TextCodec, LINES, vec![delete(quote("alpha", None, None))]);
    assert_eq!(
        String::from_utf8(out).expect("utf8"),
        "beta two\nalpha three\n"
    );
}

#[test]
fn find_none_returns_the_closest_block() {
    let base = TextCodec.project(LINES).expect("project");
    let error =
        resolve_targets(&base, vec![delete(quote("beta tree", None, None))]).expect_err("no match");
    let TargetError::FindNone { find, closest } = &error else {
        panic!("expected find_none, got {error:?}");
    };
    assert_eq!(find, "beta tree");
    let closest = closest.as_ref().expect("closest");
    assert_eq!(closest.node.as_str(), "line[2]");
    assert_eq!(closest.text, "beta two");
    let message = DocumentsError::from(error).to_string();
    assert!(
        message.contains("no block contains \"beta tree\""),
        "{message}"
    );
    assert!(message.contains("line[2]"), "{message}");
    assert!(message.contains("\"beta two\""), "{message}");
}

#[test]
fn nth_past_the_last_match_is_find_none() {
    let base = TextCodec.project(LINES).expect("project");
    let error = resolve_targets(&base, vec![delete(quote("beta", Some(2), None))])
        .expect_err("one match only");
    assert!(matches!(error, TargetError::FindNone { .. }));
}

#[test]
fn quotes_resolve_against_the_pre_call_document() {
    let ops = vec![
        EditOp::InsertAfter {
            node: NodeTarget::Id(id("line[1]")),
            kind: NodeKind::Paragraph,
            text: "beta inserted".to_owned(),
            content_as: ContentAs::Text,
        },
        delete(quote("beta", None, None)),
    ];
    let base = TextCodec.project(LINES).expect("project");
    let resolved = resolve_targets(&base, ops.clone()).expect("resolve");
    assert_eq!(resolved_node(&resolved.ops[1]), "line[2]");
    let out = resolve_and_apply(&TextCodec, LINES, ops);
    assert_eq!(
        String::from_utf8(out).expect("utf8"),
        "alpha one\nbeta inserted\nalpha three\n"
    );
}

#[test]
fn markdown_quotes_skip_the_read_only_outline() {
    let base = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let resolved =
        resolve_targets(&base, vec![delete(quote("item two", None, None))]).expect("resolve");
    assert_eq!(resolved_node(&resolved.ops[0]), "line[7]");
    assert_eq!(resolved.matches, vec![1]);
}

#[test]
fn markdown_replace_text_becomes_insert_then_delete() {
    let ops = vec![EditOp::ReplaceText {
        node: quote("item one", None, None),
        expected_hash: None,
        text: "- **first** item".to_owned(),
        content_as: ContentAs::Markdown,
    }];
    let base = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let resolved = resolve_targets(&base, ops.clone()).expect("resolve");
    assert!(matches!(
        resolved.ops.as_slice(),
        [EditOp::InsertAfter { .. }, EditOp::Delete { .. }]
    ));
    let out = String::from_utf8(resolve_and_apply(&MarkdownCodec, SAMPLE_MD, ops)).expect("utf8");
    assert!(out.contains("\n\n- **first** item\n- item two"), "{out}");
    assert!(!out.contains("item one"));
}

#[test]
fn docx_quotes_resolve_to_paragraph_ids_and_honour_within() {
    let bytes = super::docx_fixture::fixture();
    let base = DocxCodec.project(&bytes).expect("project");
    let resolved = resolve_targets(
        &base,
        vec![
            delete(quote("Revenue grew", None, None)),
            delete(quote("o", None, Some("body/tbl[0]"))),
        ],
    )
    .expect("resolve");
    assert_eq!(resolved_node(&resolved.ops[0]), "pid:2B3C4D5E");
    assert_eq!(resolved_node(&resolved.ops[1]), "pid:5E6F7081");
    let edited = resolve_and_apply(
        &DocxCodec,
        &bytes,
        vec![EditOp::ReplaceText {
            node: quote("Total", None, None),
            expected_hash: None,
            text: "Sum".to_owned(),
            content_as: ContentAs::Text,
        }],
    );
    let projection = DocxCodec.project(&edited).expect("project");
    assert_eq!(
        super::docx_fixture::text_of(&projection, "pid:6F708192"),
        "Sum"
    );
}

#[test]
fn id_targets_still_require_a_hash_on_anchored_ops() {
    let base = TextCodec.project(LINES).expect("project");
    let error = resolve_targets(&base, vec![delete(NodeTarget::Id(id("line[1]")))])
        .expect_err("hash required");
    assert!(matches!(error, TargetError::MissingHash { .. }));
}

#[test]
fn unknown_within_is_unknown_node() {
    let base = TextCodec.project(LINES).expect("project");
    let error = resolve_targets(&base, vec![delete(quote("alpha", None, Some("nope")))])
        .expect_err("unknown");
    assert!(matches!(error, TargetError::UnknownNode { .. }));
}
