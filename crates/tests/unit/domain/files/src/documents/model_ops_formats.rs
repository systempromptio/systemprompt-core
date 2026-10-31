use systemprompt_files::documents::codecs::MarkdownCodec;
use systemprompt_files::documents::codecs::odf::OdtCodec;
use systemprompt_files::documents::model::ops::apply_rich_ops;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, InsertImage, NodeTarget, Projection,
    SetBlockType, TextBlockType,
};

use super::docx_fixture::{fixture, hash_of, node_id, project};
use super::odf_fixture::{self, package};

const ODT: &str = "application/vnd.oasis.opendocument.text";

fn block_type(ids: &Projection, node: &str, block: TextBlockType) -> EditOp {
    EditOp::SetBlockType(SetBlockType {
        node: NodeTarget::Id(node_id(node)),
        expected_hash: Some(hash_of(ids, node)),
        block,
    })
}

fn unsupported(result: Result<Vec<u8>, CodecError>) -> (&'static str, DocumentFormat) {
    match result {
        Err(CodecError::UnsupportedOp { op, format }) => (op, format),
        other => panic!("expected an unsupported op, got {other:?}"),
    }
}

#[test]
fn docx_refuses_quote_and_code_block_types() {
    let bytes = fixture();
    let ids = project(&bytes);
    for block in [
        TextBlockType::Quote,
        TextBlockType::Code {
            language: Some("rust".to_owned()),
        },
    ] {
        let op = block_type(&ids, "pid:2B3C4D5E", block);
        let result = apply_rich_ops(DocumentFormat::Docx, &bytes, &ids, &[op]);
        assert_eq!(
            unsupported(result),
            ("set_block_type", DocumentFormat::Docx)
        );
    }
}

#[test]
fn odt_refuses_an_image_insert() {
    let bytes = package(ODT, "<office:text><text:p>Hello</text:p></office:text>");
    let ids = OdtCodec.project(&bytes).expect("project");
    let op = EditOp::InsertImage(InsertImage {
        after: NodeTarget::Id(odf_fixture::id("body/p[0]")),
        blob: "abc".to_owned(),
        alt: "alt".to_owned(),
    });
    let result = apply_rich_ops(DocumentFormat::Odt, &bytes, &ids, &[op]);
    assert_eq!(unsupported(result), ("insert_image", DocumentFormat::Odt));
}

#[test]
fn markdown_bytes_that_are_not_utf8_are_malformed() {
    let ids = MarkdownCodec.project(b"plain\n").expect("project");
    let result = apply_rich_ops(DocumentFormat::Markdown, b"\xff\xfe broken", &ids, &[]);
    assert!(
        matches!(
            result,
            Err(CodecError::Malformed {
                detail: "the document is not UTF-8 text"
            })
        ),
        "{result:?}"
    );
}

#[test]
fn a_format_without_a_rich_model_refuses_rich_edits() {
    let ids = MarkdownCodec.project(b"plain\n").expect("project");
    let result = apply_rich_ops(DocumentFormat::Xlsx, b"not a sheet", &ids, &[]);
    assert_eq!(
        unsupported(result),
        ("rich text edit", DocumentFormat::Xlsx)
    );
}
