use std::collections::BTreeMap;
use std::io::{Cursor, Write};

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, ImportRequest,
};
use systemprompt_files::documents::templates::TemplateError;
use systemprompt_files::{DocumentLimits, DocumentsConfig};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentRevisionId, DocumentTemplateId,
};
use systemprompt_models::documents::{ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget};
use systemprompt_provider_contracts::documents::{
    DocumentCodec, Projection, ProjectionNode, TemplateParams,
};

use std::sync::Arc;

use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::documents_service_fixture::{
    GroupSource, RoleDirectory, allow, api_author, create, harness, harness_configured, person,
    trace, unique,
};

const MIB: u64 = 1024 * 1024;

fn small_package_caps() -> DocumentsConfig {
    DocumentsConfig {
        limits: DocumentLimits {
            max_package_part_bytes: MIB,
            ..DocumentLimits::default()
        },
        ..DocumentsConfig::default()
    }
}

fn prefixed_bomb(docx: &[u8]) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(docx)).expect("open docx");
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).expect("entry");
        writer.raw_copy_file(file).expect("copy");
    }
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("word/media/bomb.bin", deflated)
        .expect("start");
    writer
        .write_all(&vec![0_u8; usize::try_from(2 * MIB).expect("fits")])
        .expect("write");
    let mut bytes = vec![b'#'];
    bytes.extend(writer.finish().expect("finish").into_inner());
    bytes
}

fn is_part_too_large(error: &DocumentsError) -> bool {
    matches!(
        error,
        DocumentsError::TooLarge {
            what: "zip package part (inflated bytes)",
            limit,
            ..
        } if *limit == MIB
    )
}

fn commit_bytes(document: &DocumentId, base: &DocumentRevisionId, bytes: Vec<u8>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: Some(base.clone()),
        body: CommitBody::Bytes(bytes),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

fn find<'p>(nodes: &'p [ProjectionNode], text: &str) -> Option<&'p ProjectionNode> {
    nodes.iter().find_map(|node| {
        if node.children.is_empty() && node.text.as_deref() == Some(text) {
            Some(node)
        } else {
            find(&node.children, text)
        }
    })
}

fn path_of(projection: &Projection, text: &str) -> DocumentNodeId {
    find(&projection.root, text)
        .unwrap_or_else(|| panic!("no leaf reads {text:?}"))
        .id
        .clone()
}

fn insert_after(bytes: &[u8], anchor: &str, text: &str) -> Vec<u8> {
    let projection = DocxCodec.project(bytes).expect("project");
    let op = EditOp::InsertAfter {
        node: NodeTarget::Id(path_of(&projection, anchor)),
        kind: NodeKind::Paragraph,
        text: text.to_owned(),
        content_as: ContentAs::Text,
    };
    DocxCodec.apply(bytes, &projection, &[op]).expect("insert")
}

fn replace(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let projection = DocxCodec.project(bytes).expect("project");
    let node = find(&projection.root, from).expect("leaf");
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(node.id.clone()),
        expected_hash: Some(node.hash8.clone()),
        text: to.to_owned(),
        content_as: ContentAs::Text,
    };
    DocxCodec.apply(bytes, &projection, &[op]).expect("replace")
}

#[tokio::test]
async fn a_stale_office_upload_adding_adjacent_paragraphs_merges_in_order() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "Alpha\n\nOmega".to_owned(),
    }];
    let (document, first) = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create docx");
    let (_, base) = h
        .service
        .read_bytes(&owner, &document.id, None, &trace())
        .await
        .expect("read base");

    let head = h
        .service
        .commit(
            &owner,
            commit_bytes(
                &document.id,
                &first.id,
                replace(&base, "Omega", "Omega edited"),
            ),
            &trace(),
        )
        .await
        .expect("head commit");
    assert!(matches!(head, CommitOutcome::Committed(_)), "{head:?}");

    let upload = insert_after(&insert_after(&base, "Alpha", "One"), "One", "Two");
    let outcome = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, upload),
            &trace(),
        )
        .await
        .expect("stale upload");
    let CommitOutcome::Merged(landed) = outcome else {
        panic!("expected Merged, got {outcome:?}");
    };
    let (_, merged) = h
        .service
        .read_bytes(&owner, &document.id, Some(&landed.revision.id), &trace())
        .await
        .expect("read merged");
    let text = DocxCodec.plain_text(&merged).expect("plain text");
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {text}"))
    };
    assert!(at("Alpha") < at("One"), "{text}");
    assert!(at("One") < at("Two"), "{text}");
    assert!(at("Two") < at("Omega edited"), "{text}");
}

#[tokio::test]
async fn a_template_create_with_too_many_ops_is_too_large() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let limit = h.service.config().limits.max_ops_per_call;
    let mut request = create("blank-docx", None);
    request.ops = (0..=limit)
        .map(|n| EditOp::AppendMarkdown {
            markdown: format!("p{n}"),
        })
        .collect();
    let error = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect_err("refused");
    assert!(
        matches!(
            error,
            DocumentsError::TooLarge {
                what: "ops in one call",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_template_create_with_too_many_params_is_too_large() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let mut request = create("blank-markdown", None);
    request.params = TemplateParams {
        values: (0..300)
            .map(|n| (format!("k{n}"), "v".to_owned()))
            .collect::<BTreeMap<_, _>>(),
    };
    let error = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect_err("refused");
    assert!(
        matches!(
            error,
            DocumentsError::TooLarge {
                what: "template parameters",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_unknown_template_is_a_template_not_found() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let mut request = create("blank-docx", None);
    request.template_id = DocumentTemplateId::new("no-such-template");
    let error = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect_err("refused");
    assert!(
        matches!(
            error,
            DocumentsError::Template {
                source: TemplateError::NotFound { .. },
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_byte_commit_over_the_upload_limit_is_refused_before_it_is_parsed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("owner");
    let (document, first) = h
        .service
        .create_from_template(&owner, create("blank-docx", None), &trace())
        .await
        .expect("create docx");
    let limit = h.service.config().max_upload_bytes;
    let oversized = vec![b'x'; usize::try_from(limit).expect("limit fits") + 1];
    let error = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, oversized),
            &trace(),
        )
        .await
        .expect_err("refused");
    assert!(
        matches!(
            error,
            DocumentsError::TooLarge {
                what: "document content (bytes)",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_prefixed_package_bomb_is_refused_on_import() {
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        small_package_caps(),
    )
    .await;
    let owner = person("owner");
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-docx", None), &trace())
        .await
        .expect("create docx");
    let (_, docx) = h
        .service
        .read_bytes(&owner, &document.id, None, &trace())
        .await
        .expect("read docx");
    let error = h
        .service
        .import(
            &owner,
            ImportRequest {
                folder: None,
                name: format!("{}.docx", unique("bomb")),
                format: Some(DocumentFormat::Docx),
                mime_type: None,
                bytes: prefixed_bomb(&docx),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect_err("refused");
    assert!(is_part_too_large(&error), "{error:?}");
}

#[tokio::test]
async fn a_prefixed_package_bomb_is_refused_as_a_stale_upload() {
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        small_package_caps(),
    )
    .await;
    let owner = person("owner");
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "Alpha".to_owned(),
    }];
    let (document, first) = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create docx");
    let (_, docx) = h
        .service
        .read_bytes(&owner, &document.id, None, &trace())
        .await
        .expect("read docx");
    let head = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, replace(&docx, "Alpha", "Moved on")),
            &trace(),
        )
        .await
        .expect("head commit");
    assert!(matches!(head, CommitOutcome::Committed(_)), "{head:?}");
    let error = h
        .service
        .commit(
            &owner,
            commit_bytes(&document.id, &first.id, prefixed_bomb(&docx)),
            &trace(),
        )
        .await
        .expect_err("refused");
    assert!(is_part_too_large(&error), "{error:?}");
}
