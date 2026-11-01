use std::sync::Arc;

use futures::StreamExt;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_files::documents::service::{FileCreateRequest, ImportRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use tempfile::TempDir;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, api_author, create, deny, harness,
    harness_configured, person, trace, unique,
};

const PDF: &[u8] = b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n";

async fn markdown(h: &Harness, owner: &AuthenticatedUser, name: &str, text: &str) -> DocumentId {
    h.service
        .import(
            owner,
            ImportRequest {
                folder: None,
                name: name.to_owned(),
                format: Some(DocumentFormat::Markdown),
                mime_type: None,
                bytes: text.as_bytes().to_vec(),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("import markdown")
        .0
        .id
}

async fn file(h: &Harness, owner: &AuthenticatedUser, name: &str, bytes: &[u8]) -> DocumentId {
    h.service
        .create_from_file(
            owner,
            FileCreateRequest {
                name: name.to_owned(),
                folder: None,
                format: Some(DocumentFormat::File),
                mime: "application/octet-stream".to_owned(),
                bytes: bytes.to_vec(),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("create from file")
        .0
        .id
}

async fn drain(mut body: systemprompt_traits::StoredFileRead) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.body.next().await {
        bytes.extend_from_slice(&chunk.expect("chunk"));
    }
    bytes
}

#[tokio::test]
async fn a_rich_document_renders_to_a_pdf_named_after_its_stem() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-render");
    let stem = unique("report");
    let document = markdown(&h, &owner, &format!("{stem}.md"), "# Title\n\nBody text.\n").await;
    let rendered = h
        .service
        .render_pdf(&owner, &document, None, &trace())
        .await
        .expect("render");
    assert_eq!(rendered.file_name, format!("{stem}.pdf"));
    assert!(rendered.bytes.starts_with(b"%PDF-"), "not a pdf");
    let (head, _) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("head");
    assert_eq!(rendered.revision.id, head.id);

    let again = h
        .service
        .render_pdf(&owner, &document, Some(&head.id), &trace())
        .await
        .expect("render pinned revision");
    assert_eq!(again.revision.id, head.id);
    assert_eq!(again.file_name, rendered.file_name);
}

#[tokio::test]
async fn templates_in_each_rich_format_render() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-formats");
    for template in ["blank-docx", "blank-odt", "blank-html", "blank-text"] {
        let mut request = create(template, None);
        if template != "blank-text" {
            request.content = Some(RichText::of(vec![Block::new(
                "p1",
                BlockKind::Paragraph,
                "Rendered paragraph",
            )]));
        }
        let document = h
            .service
            .create_from_template(&owner, request, &trace())
            .await
            .unwrap_or_else(|error| panic!("{template}: {error}"))
            .0;
        let rendered = h
            .service
            .render_pdf(&owner, &document.id, None, &trace())
            .await
            .unwrap_or_else(|error| panic!("{template}: {error}"));
        assert!(rendered.bytes.starts_with(b"%PDF-"), "{template}");
        assert!(rendered.file_name.ends_with(".pdf"), "{template}");
    }
}

#[tokio::test]
async fn the_page_cap_refuses_a_document_longer_than_the_limit() {
    let mut config = DocumentsConfig::default();
    config.limits.pdf_pages = 1;
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        config,
    )
    .await;
    let owner = person("pdf-cap");
    let long: String = (0..400)
        .map(|at| format!("Paragraph number {at}.\n\n"))
        .collect();
    let document = markdown(&h, &owner, &unique("long"), &long).await;
    let error = h
        .service
        .render_pdf(&owner, &document, None, &trace())
        .await
        .expect_err("over the page cap");
    match error {
        DocumentsError::TooLarge { what, size, limit } => {
            assert_eq!(what, "rendered PDF (pages)");
            assert_eq!((size, limit), (2, 1));
        },
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn a_stored_file_is_not_rendered_but_a_pdf_file_streams_whole() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-file");
    let stored = file(&h, &owner, &format!("{}.pdf", unique("scan")), PDF).await;
    let refused = h
        .service
        .render_pdf(&owner, &stored, None, &trace())
        .await
        .expect_err("a file is not rendered");
    assert!(
        matches!(
            refused,
            DocumentsError::UnsupportedFormat {
                format: DocumentFormat::File
            }
        ),
        "{refused:?}"
    );

    let (revision, read) = h
        .service
        .open_pdf_stream(&owner, &stored, None, &trace())
        .await
        .expect("open")
        .expect("a pdf streams");
    let (head, _) = h
        .service
        .read_bytes(&owner, &stored, None, &trace())
        .await
        .expect("head");
    assert_eq!(revision.id, head.id);
    assert_eq!(drain(read).await, PDF);
}

#[tokio::test]
async fn non_pdf_and_tiny_files_open_no_pdf_stream() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-none");
    let binary = file(
        &h,
        &owner,
        &unique("blob"),
        b"\x00\x01\x02 not a pdf at all",
    )
    .await;
    assert!(
        h.service
            .open_pdf_stream(&owner, &binary, None, &trace())
            .await
            .expect("open")
            .is_none()
    );
    let tiny = file(&h, &owner, &unique("tiny"), b"%P").await;
    assert!(
        h.service
            .open_pdf_stream(&owner, &tiny, None, &trace())
            .await
            .expect("open tiny")
            .is_none()
    );
}

#[tokio::test]
async fn a_viewer_without_access_can_neither_render_nor_stream() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-owner");
    let document = markdown(&h, &owner, &unique("private"), "secret\n").await;
    let stored = file(&h, &owner, &format!("{}.pdf", unique("private")), PDF).await;
    let denied = harness(deny(), GroupSource::new(&[])).await;
    let stranger = person("pdf-stranger");
    let error = denied
        .service
        .render_pdf(&stranger, &document, None, &trace())
        .await
        .expect_err("stranger render");
    assert!(
        matches!(
            error,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{error:?}"
    );
    let error = denied
        .service
        .open_pdf_stream(&stranger, &stored, None, &trace())
        .await
        .expect_err("stranger stream");
    assert!(
        matches!(
            error,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_file_that_does_not_import_is_refused_and_its_name_picks_the_format() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("file-create");
    let request = |name: String, bytes: &[u8]| FileCreateRequest {
        name,
        folder: None,
        format: None,
        mime: "application/octet-stream".to_owned(),
        bytes: bytes.to_vec(),
        author: api_author(),
    };
    let error = h
        .service
        .create_from_file(
            &owner,
            request(format!("{}.docx", unique("bad")), b"not a zip"),
            &trace(),
        )
        .await
        .expect_err("garbage docx");
    assert!(
        !matches!(error, DocumentsError::NotFound { .. }),
        "{error:?}"
    );
    let (document, _) = h
        .service
        .create_from_file(
            &owner,
            request(format!("{}.md", unique("notes")), b"# Heading\n"),
            &trace(),
        )
        .await
        .expect("markdown by name");
    assert_eq!(document.format, DocumentFormat::Markdown);
}
