use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, NodeFragment, NodeKind, NodeTarget,
    Projection, ProjectionNode, TemplateParams, content_hash,
};

#[derive(Debug)]
struct TreeCodec;

fn leaf(id: &str, text: &str, hash: &str) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind: NodeKind::Paragraph,
        text: Some(text.to_owned()),
        hash8: hash.to_owned(),
        children: Vec::new(),
        attrs: BTreeMap::new(),
    }
}

impl DocumentCodec for TreeCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Text]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        Ok(Projection {
            format: DocumentFormat::Text,
            root: vec![ProjectionNode {
                id: DocumentNodeId::new("body"),
                kind: NodeKind::Body,
                text: None,
                hash8: "bodyhash".to_owned(),
                children: vec![
                    leaf("p1", "first", "hash0001"),
                    leaf("p2", "second", "hash0002"),
                ],
                attrs: BTreeMap::new(),
            }],
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        _bytes: &[u8],
        _base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        Ok(serde_json::to_vec(ops).expect("ops serialise"))
    }

    fn preview_html(&self, _: &[u8]) -> Result<String, CodecError> {
        Ok(String::new())
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn instantiate(&self, template: &[u8], _: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        Ok(template.to_vec())
    }
}

#[test]
fn style_hashes_cover_only_leaf_nodes() {
    let hashes = TreeCodec.style_hashes(b"doc").expect("hashes");
    assert_eq!(
        hashes,
        vec![
            (DocumentNodeId::new("p1"), "hash0001".to_owned()),
            (DocumentNodeId::new("p2"), "hash0002".to_owned()),
        ]
    );
}

#[test]
fn copy_node_copies_a_leaf_and_refuses_a_container() {
    let base = TreeCodec.project(b"doc").expect("projection");
    let fragment = TreeCodec
        .copy_node(b"doc", &base, &DocumentNodeId::new("p2"))
        .expect("leaf");
    assert_eq!(fragment.text.as_deref(), Some("second"));
    assert!(fragment.native.is_none());
    assert!(matches!(
        TreeCodec.copy_node(b"doc", &base, &DocumentNodeId::new("body")),
        Err(CodecError::UnsupportedOp {
            op: "copy_node",
            ..
        })
    ));
    assert!(matches!(
        TreeCodec.copy_node(b"doc", &base, &DocumentNodeId::new("p9")),
        Err(CodecError::UnknownNode { .. })
    ));
}

#[test]
fn insert_fragment_applies_a_text_insert_after_the_target() {
    let base = TreeCodec.project(b"doc").expect("projection");
    let fragment = NodeFragment {
        kind: NodeKind::Paragraph,
        text: Some("inserted".to_owned()),
        native: None,
    };
    let applied = TreeCodec
        .insert_fragment(
            b"doc",
            &base,
            &NodeTarget::Id(DocumentNodeId::new("p1")),
            &fragment,
        )
        .expect("applied");
    let ops: serde_json::Value = serde_json::from_slice(&applied).expect("json");
    let rendered = ops.to_string();
    assert_eq!(ops.as_array().map(Vec::len), Some(1));
    assert!(rendered.contains("inserted"), "{rendered}");
    assert!(rendered.contains("p1"), "{rendered}");
}

#[test]
fn insert_fragment_refuses_a_native_fragment() {
    let base = TreeCodec.project(b"doc").expect("projection");
    let fragment = NodeFragment {
        kind: NodeKind::Paragraph,
        text: None,
        native: Some(b"<w:p/>".to_vec()),
    };
    let err = TreeCodec
        .insert_fragment(
            b"doc",
            &base,
            &NodeTarget::Id(DocumentNodeId::new("p1")),
            &fragment,
        )
        .expect_err("native refused");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp {
            op: "insert_fragment",
            format: DocumentFormat::Text
        }
    ));
}

#[test]
fn defaults_declare_no_insert_kinds() {
    assert!(TreeCodec.insert_kinds(DocumentFormat::Text).is_empty());
}
