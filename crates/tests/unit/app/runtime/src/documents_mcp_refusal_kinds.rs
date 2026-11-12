use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::LockHolder;
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentNodeId, DriveDeviceId};
use systemprompt_mcp::McpError;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};
use systemprompt_runtime::documents::mcp::{refusal_result, tool_error};

fn data(error: DocumentsError) -> (String, serde_json::Value) {
    let mcp = tool_error(error);
    let result = refusal_result(&mcp).expect("a refusal is a tool result");
    assert_eq!(result.is_error, Some(true));
    let text = serde_json::to_value(&result.content[0])
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_owned))
        .expect("text block");
    (text, result.structured_content.expect("structured refusal"))
}

fn code(error: DocumentsError) -> String {
    data(error).1["code"].as_str().expect("code").to_owned()
}

fn internal(error: DocumentsError) -> McpError {
    let mcp = tool_error(error);
    assert!(refusal_result(&mcp).is_none());
    mcp
}

#[test]
fn missing_things_are_not_found() {
    assert_eq!(
        code(DocumentsError::NotFound {
            what: "document",
            id: "d-1".to_owned()
        }),
        "not_found"
    );
    assert_eq!(
        code(DocumentsError::CommentNotFound {
            document_id: DocumentId::generate(),
            comment_id: DocumentCommentId::generate(),
        }),
        "not_found"
    );
    let (_, d) = data(DocumentsError::UnknownNode {
        node: DocumentNodeId::new("n-9"),
    });
    assert_eq!(d["code"], "not_found");
    assert_eq!(d["node"], "n-9");
}

#[test]
fn a_held_lock_is_locked() {
    assert_eq!(
        code(DocumentsError::LockHeld {
            document_id: DocumentId::generate(),
            holder: LockHolder::Bridge(DriveDeviceId::new("device-1")),
        }),
        "locked"
    );
}

#[test]
fn find_none_names_the_closest_block_when_there_is_one() {
    let (text, d) = data(DocumentsError::FindNone {
        find: "quoted".to_owned(),
        closest: Some(ClosestBlock {
            node: DocumentNodeId::new("p-2"),
            hash: "abcd1234".to_owned(),
            text: "near quote".to_owned(),
        }),
    });
    assert_eq!(d["code"], "find_none");
    assert_eq!(d["matches"], 0);
    assert_eq!(d["node"], "p-2");
    assert_eq!(d["hash"], "abcd1234");
    assert_eq!(d["current_text"], "near quote");
    assert!(text.contains("quoted"), "{text}");

    let (_, d) = data(DocumentsError::FindNone {
        find: "quoted".to_owned(),
        closest: None,
    });
    assert_eq!(d["matches"], 0);
    assert!(d.get("node").is_none_or(serde_json::Value::is_null));
}

#[test]
fn format_and_parameter_errors_have_their_codes() {
    assert_eq!(
        code(DocumentsError::UnsupportedFormat {
            format: DocumentFormat::Xlsx
        }),
        "unsupported_format"
    );
    assert_eq!(
        code(DocumentsError::UnknownFormat {
            name: "odt".to_owned()
        }),
        "unsupported_format"
    );
    assert_eq!(
        code(DocumentsError::TooLarge {
            what: "document",
            size: 10,
            limit: 1
        }),
        "too_large"
    );
    assert_eq!(
        code(DocumentsError::InvalidName {
            name: "..".to_owned()
        }),
        "invalid_params"
    );
    assert_eq!(
        code(DocumentsError::UnknownSubjectKind {
            kind: "robot".to_owned()
        }),
        "invalid_params"
    );
    assert_eq!(
        code(DocumentsError::InvalidComment { detail: "empty" }),
        "invalid_params"
    );
}

#[test]
fn codec_refusals_map_to_their_codes() {
    let (_, d) = data(DocumentsError::Codec(CodecError::UnsupportedOp {
        op: "merge_cells",
        format: DocumentFormat::Docx,
    }));
    assert_eq!(d["code"], "unsupported_op");

    let (_, d) = data(DocumentsError::Codec(CodecError::UnknownNode {
        node: DocumentNodeId::new("gone"),
    }));
    assert_eq!(d["code"], "not_found");
    assert_eq!(d["node"], "gone");

    let (text, d) = data(DocumentsError::Codec(CodecError::StaleAnchor {
        node: DocumentNodeId::new("p-1"),
        expected: "old".to_owned(),
        actual: "new".to_owned(),
        current_text: "now".to_owned(),
    }));
    assert_eq!(d["code"], "guard_mismatch");
    assert_eq!(d["hash"], "new");
    assert_eq!(d["current_text"], "now");
    assert!(text.contains("p-1"), "{text}");

    let (text, d) = data(DocumentsError::Codec(CodecError::Malformed {
        detail: "no body",
    }));
    assert_eq!(d["code"], "invalid_params");
    assert!(text.contains("no body"), "{text}");
}

#[test]
fn a_codec_io_failure_stays_internal_and_a_parse_failure_is_refused() {
    let io = internal(DocumentsError::Codec(CodecError::Io(
        std::io::Error::other("disk"),
    )));
    assert_eq!(io.message, "the document store failed; try the call again");
    let (text, d) = data(DocumentsError::Codec(CodecError::Parse {
        part: "word/document.xml".to_owned(),
        source: "bad xml".into(),
    }));
    assert_eq!(d["code"], "invalid_params");
    assert!(text.contains("word/document.xml"), "{text}");
}

#[test]
fn store_failures_are_generic_internal_errors() {
    let error = internal(DocumentsError::Repository(
        systemprompt_traits::RepositoryError::NotFound {
            entity: "row",
            key: None,
        },
    ));
    assert_eq!(
        error.message,
        "the document store failed; try the call again"
    );
}
