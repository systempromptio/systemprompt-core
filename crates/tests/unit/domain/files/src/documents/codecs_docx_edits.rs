use systemprompt_files::documents::codecs::docx::{DocxCodec, blank_docx};
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, EditOp, NodeKind};

use super::docx_fixture::{apply, fixture, hash_of, node_id, part, project, text_of};

fn str_replace(bytes: &[u8], old: &str, new: &str) -> Result<Vec<u8>, CodecError> {
    DocxCodec.apply(
        bytes,
        &project(bytes),
        &[EditOp::StrReplace {
            old_str: old.to_owned(),
            new_str: new.to_owned(),
        }],
    )
}

#[test]
fn stale_hash_reports_current_text() {
    let bytes = fixture();
    let err = DocxCodec
        .apply(
            &bytes,
            &project(&bytes),
            &[EditOp::ReplaceText {
                node: node_id("pid:2B3C4D5E"),
                expected_hash: "00000000".to_owned(),
                text: "x".to_owned(),
            }],
        )
        .expect_err("stale");
    match err {
        CodecError::StaleAnchor {
            current_text,
            expected,
            ..
        } => {
            assert_eq!(current_text, "Revenue grew strongly.");
            assert_eq!(expected, "00000000");
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn para_ids_stay_stable_across_an_insert() {
    let bytes = fixture();
    let before = project(&bytes);
    let edited = apply(
        &bytes,
        &[EditOp::InsertAfter {
            node: node_id("pid:1A2B3C4D"),
            kind: NodeKind::Paragraph,
            text: "Inserted".to_owned(),
        }],
    );
    let after = project(&edited);
    assert_eq!(
        hash_of(&before, "pid:3C4D5E6F"),
        hash_of(&after, "pid:3C4D5E6F")
    );
    let inserted = &after.root[1];
    assert_eq!(inserted.text.as_deref(), Some("Inserted"));
    assert_eq!(inserted.kind, NodeKind::Paragraph);
    let pid = inserted.attrs.get("para_id").expect("fresh para id");
    assert_eq!(pid.len(), 8);
    assert!(u32::from_str_radix(pid, 16).expect("hex") < 0x8000_0000);
    assert_eq!(after.root.len(), before.root.len() + 1);
}

#[test]
fn insert_heading_uses_the_heading_style() {
    let bytes = fixture();
    let edited = apply(
        &bytes,
        &[
            EditOp::InsertAfter {
                node: node_id("pid:2B3C4D5E"),
                kind: NodeKind::Heading { level: 2 },
                text: "Outlook".to_owned(),
            },
            EditOp::InsertAfter {
                node: node_id("pid:3C4D5E6F"),
                kind: NodeKind::ListItem,
                text: "Second point".to_owned(),
            },
        ],
    );
    let after = project(&edited);
    assert_eq!(after.root[2].kind, NodeKind::Heading { level: 2 });
    assert_eq!(after.root[2].text.as_deref(), Some("Outlook"));
    assert_eq!(after.root[4].kind, NodeKind::ListItem);
    assert_eq!(after.root[4].text.as_deref(), Some("Second point"));
}

#[test]
fn batch_ids_resolve_against_the_base_revision() {
    let bytes = blank_docx().expect("blank");
    let base = project(&bytes);
    let empty = hash_of(&base, "body/p[0]");
    let edited = apply(
        &bytes,
        &[
            EditOp::InsertAfter {
                node: node_id("body/p[0]"),
                kind: NodeKind::Paragraph,
                text: "one".to_owned(),
            },
            EditOp::InsertAfter {
                node: node_id("body/p[0]"),
                kind: NodeKind::Paragraph,
                text: "two".to_owned(),
            },
            EditOp::ReplaceText {
                node: node_id("body/p[0]"),
                expected_hash: empty,
                text: "zero".to_owned(),
            },
        ],
    );
    let texts: Vec<_> = project(&edited)
        .root
        .iter()
        .map(|n| n.text.clone().unwrap_or_default())
        .collect();
    assert_eq!(texts, ["zero", "one", "two"]);
}

#[test]
fn conflicting_ops_on_one_node_fail_the_batch() {
    let bytes = fixture();
    let base = project(&bytes);
    let hash = hash_of(&base, "pid:1A2B3C4D");
    let op = EditOp::ReplaceText {
        node: node_id("pid:1A2B3C4D"),
        expected_hash: hash,
        text: "x".to_owned(),
    };
    let err = DocxCodec
        .apply(&bytes, &base, &[op.clone(), op])
        .expect_err("conflict");
    assert!(matches!(err, CodecError::Malformed { .. }));
}

#[test]
fn delete_keeps_a_paragraph_in_every_cell() {
    let bytes = fixture();
    let base = project(&bytes);
    let edited = apply(
        &bytes,
        &[
            EditOp::Delete {
                node: node_id("pid:5E6F7081"),
                expected_hash: hash_of(&base, "pid:5E6F7081"),
            },
            EditOp::Delete {
                node: node_id("pid:3C4D5E6F"),
                expected_hash: hash_of(&base, "pid:3C4D5E6F"),
            },
        ],
    );
    let after = project(&edited);
    assert!(after.find(&node_id("pid:3C4D5E6F")).is_none());
    assert_eq!(text_of(&after, "pid:5E6F7081"), "");
    assert!(part(&edited, "word/document.xml").contains(r#"<w:p w14:paraId="5E6F7081"></w:p>"#));
}

#[test]
fn str_replace_unique_ambiguous_spanning_and_missing() {
    let bytes = fixture();
    let edited = str_replace(&bytes, "strongly", "steadily").expect("unique");
    assert_eq!(
        text_of(&project(&edited), "pid:2B3C4D5E"),
        "Revenue grew steadily."
    );
    assert!(matches!(
        str_replace(&bytes, "{{client}}", "x"),
        Err(CodecError::AmbiguousMatch { matches: 2 })
    ));
    assert!(matches!(
        str_replace(&bytes, "Report\nRevenue", "x"),
        Err(CodecError::AmbiguousMatch { .. })
    ));
    assert!(matches!(
        str_replace(&bytes, "absent", "x"),
        Err(CodecError::NoMatch)
    ));
}

#[test]
fn unsupported_ops_and_unknown_nodes() {
    let bytes = fixture();
    let base = project(&bytes);
    let err = DocxCodec
        .apply(
            &bytes,
            &base,
            &[EditOp::SetText {
                text: "x".to_owned(),
            }],
        )
        .expect_err("unsupported");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp { op: "set_text", .. }
    ));
    let err = DocxCodec
        .apply(
            &bytes,
            &base,
            &[EditOp::Delete {
                node: node_id("pid:FFFFFFFF"),
                expected_hash: "00000000".to_owned(),
            }],
        )
        .expect_err("unknown");
    assert!(matches!(err, CodecError::UnknownNode { .. }));
    let other = blank_docx().expect("blank");
    let err = DocxCodec
        .apply(
            &other,
            &base,
            &[EditOp::SetText {
                text: "x".to_owned(),
            }],
        )
        .expect_err("base mismatch");
    assert!(matches!(err, CodecError::Malformed { .. }));
}
