use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{CellValue, EditOp, NodeKind};

fn all_ops() -> Vec<EditOp> {
    let node = DocumentNodeId::new("p1");
    vec![
        EditOp::ReplaceText {
            node: node.clone(),
            expected_hash: "abcd1234".into(),
            text: "new".into(),
        },
        EditOp::InsertAfter {
            node: node.clone(),
            kind: NodeKind::Heading { level: 2 },
            text: "h".into(),
        },
        EditOp::Delete {
            node: node.clone(),
            expected_hash: "abcd1234".into(),
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
