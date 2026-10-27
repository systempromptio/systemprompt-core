use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{
    CellValue, ContentAs, EditOp, NodeKind, NodeTarget, QuoteTarget,
};

fn all_ops() -> Vec<EditOp> {
    let node = DocumentNodeId::new("p1");
    vec![
        EditOp::ReplaceText {
            node: NodeTarget::Id(node.clone()),
            expected_hash: Some("abcd1234".into()),
            text: "new".into(),
            content_as: ContentAs::Text,
        },
        EditOp::InsertAfter {
            node: NodeTarget::Id(node.clone()),
            kind: NodeKind::Heading { level: 2 },
            text: "h".into(),
            content_as: ContentAs::Text,
        },
        EditOp::Delete {
            node: NodeTarget::Id(node.clone()),
            expected_hash: Some("abcd1234".into()),
        },
        EditOp::SetCells {
            sheet: "Sheet1".into(),
            range: "A1:C1".into(),
            values: vec![vec![
                CellValue::Text("x".into()),
                CellValue::Number(1.5),
                CellValue::Bool(true),
                CellValue::Empty,
            ]],
            expected_hash: None,
        },
        EditOp::InsertSlide {
            after: Some(node),
            layout: Some("title".into()),
            title: Some("T".into()),
            body: vec!["b".into()],
        },
        EditOp::StrReplace {
            old_str: "a".into(),
            new_str: "b".into(),
        },
        EditOp::SetHtml {
            html: "<p>x</p>".into(),
        },
        EditOp::SetText { text: "x".into() },
        EditOp::AppendMarkdown {
            markdown: "# x".into(),
        },
    ]
}

#[test]
fn every_op_round_trips_with_its_tag() {
    let tags = [
        "replace_text",
        "insert_after",
        "delete",
        "set_cells",
        "insert_slide",
        "str_replace",
        "set_html",
        "set_text",
        "append_markdown",
    ];
    for (op, tag) in all_ops().into_iter().zip(tags) {
        let json = serde_json::to_value(&op).expect("serialize");
        assert_eq!(json["op"], tag);
        assert_eq!(op.name(), tag);
        let back: EditOp = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, op);
    }
}

#[test]
fn cell_values_serialise_untagged() {
    let json = serde_json::to_value(vec![
        CellValue::Text("x".into()),
        CellValue::Number(2.0),
        CellValue::Bool(false),
        CellValue::Empty,
    ])
    .expect("serialize");
    assert_eq!(json, serde_json::json!(["x", 2.0, false, null]));
}

#[test]
fn touches_reports_anchor_node() {
    let ops = all_ops();
    assert_eq!(ops[0].touches().map(DocumentNodeId::as_str), Some("p1"));
    assert_eq!(ops[4].touches().map(DocumentNodeId::as_str), Some("p1"));
    assert!(ops[5].touches().is_none());
    assert!(ops[3].touches().is_none());
}

#[test]
fn quote_target_and_markdown_content_parse_from_the_wire() {
    let op: EditOp = serde_json::from_value(serde_json::json!({
        "op": "insert_after",
        "target": {"find": "Revenue grew", "nth": 2, "within": "body/tbl[0]"},
        "text": "**bold**",
        "as": "markdown"
    }))
    .expect("deserialize");
    let EditOp::InsertAfter {
        node,
        kind,
        content_as,
        ..
    } = &op
    else {
        panic!("expected insert_after, got {op:?}");
    };
    assert_eq!(
        node,
        &NodeTarget::Find(QuoteTarget {
            find: "Revenue grew".into(),
            nth: std::num::NonZeroUsize::new(2),
            within: Some(DocumentNodeId::new("body/tbl[0]")),
        })
    );
    assert_eq!(*kind, NodeKind::Paragraph);
    assert_eq!(*content_as, ContentAs::Markdown);
    assert!(op.touches().is_none());
}

#[test]
fn quote_targets_need_no_expected_hash() {
    let op: EditOp = serde_json::from_value(serde_json::json!({
        "op": "delete",
        "target": {"find": "obsolete"}
    }))
    .expect("deserialize");
    assert!(matches!(
        op,
        EditOp::Delete {
            node: NodeTarget::Find(_),
            expected_hash: None,
        }
    ));
}

#[test]
fn text_content_is_the_default_and_omitted_on_the_wire() {
    let op: EditOp = serde_json::from_value(serde_json::json!({
        "op": "replace_text",
        "node": "p1",
        "expected_hash": "abcd1234",
        "text": "x"
    }))
    .expect("deserialize");
    let json = serde_json::to_value(&op).expect("serialize");
    assert!(json.get("as").is_none());
    assert_eq!(json["node"], "p1");
}

#[test]
fn unresolved_ops_are_refused_by_check_resolved() {
    let quoted = EditOp::Delete {
        node: NodeTarget::Find(QuoteTarget {
            find: "x".into(),
            nth: None,
            within: None,
        }),
        expected_hash: None,
    };
    assert!(quoted.check_resolved().is_err());
    let markdown_replace = EditOp::ReplaceText {
        node: NodeTarget::Id(DocumentNodeId::new("p1")),
        expected_hash: Some("abcd1234".into()),
        text: "# x".into(),
        content_as: ContentAs::Markdown,
    };
    assert!(markdown_replace.check_resolved().is_err());
    assert!(all_ops().iter().all(|op| op.check_resolved().is_ok()));
}
