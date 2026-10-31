use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::import_rich;
use systemprompt_files::documents::model::{
    Block, BlockKind, DocumentSettings, Mark, MarkKind, Media, RichText,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;
use systemprompt_provider_contracts::documents::CodecError;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace,
};

fn body() -> RichText {
    RichText::of(vec![
        Block::new("h1", BlockKind::Heading { level: 1 }, "Quarterly review"),
        Block::new("p1", BlockKind::Paragraph, "Revenue grew strongly."),
        Block::new(
            "l1",
            BlockKind::ListItem {
                level: 0,
                ordered: false,
                numbering: None,
                checked: None,
            },
            "Hire two engineers",
        ),
    ])
}

async fn made(
    h: &Harness,
    owner: &AuthenticatedUser,
    template: &str,
    content: RichText,
) -> Result<DocumentId, DocumentsError> {
    let mut request = create(template, None);
    request.content = Some(content);
    Ok(h.service
        .create_from_template(owner, request, &trace())
        .await?
        .0
        .id)
}

async fn text_of(h: &Harness, owner: &AuthenticatedUser, document: &DocumentId) -> String {
    h.service
        .plain_text(owner, document, None, &trace())
        .await
        .expect("plain text")
        .1
}

fn refused_detail(error: DocumentsError) -> String {
    match error {
        DocumentsError::Codec(CodecError::Refused { detail }) => detail,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn content_replaces_the_template_body_in_every_rich_format() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-formats");
    for template in ["blank-docx", "blank-odt", "blank-markdown", "blank-html"] {
        let document = made(&h, &owner, template, body())
            .await
            .unwrap_or_else(|error| panic!("{template}: {error}"));
        let text = text_of(&h, &owner, &document).await;
        for line in [
            "Quarterly review",
            "Revenue grew strongly.",
            "Hire two engineers",
        ] {
            assert!(text.contains(line), "{template} lacks {line}: {text:?}");
        }
    }
    let (_, markdown) = {
        let document = made(&h, &owner, "blank-markdown", body())
            .await
            .expect("md");
        h.service
            .read_bytes(&owner, &document, None, &trace())
            .await
            .expect("bytes")
    };
    let markdown = String::from_utf8(markdown).expect("utf8");
    assert!(markdown.contains("# Quarterly review"), "{markdown}");
    assert!(markdown.contains("- Hire two engineers"), "{markdown}");
}

#[tokio::test]
async fn content_settings_override_and_join_the_template_settings() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-settings");
    let template_settings = {
        let document = made(&h, &owner, "blank-docx", body()).await.expect("plain");
        let (_, bytes) = h
            .service
            .read_bytes(&owner, &document, None, &trace())
            .await
            .expect("bytes");
        import_rich(&bytes).expect("import").settings()
    };
    let held = template_settings
        .styles
        .first()
        .cloned()
        .expect("a template style");

    let mut settings = DocumentSettings::default();
    let mut renamed = held.clone();
    renamed.name = "Renamed by content".to_owned();
    renamed.bold = true;
    settings.styles.push(renamed);
    let mut added = held.clone();
    added.id = "SpAddedStyle".to_owned();
    added.name = "Added by content".to_owned();
    settings.styles.push(added);
    let mut content = body();
    content.document = Some(Box::new(settings));
    let document = made(&h, &owner, "blank-docx", content)
        .await
        .expect("styled");
    let (_, bytes) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("bytes");
    let styles = import_rich(&bytes).expect("import").settings().styles;
    let by_id = |id: &str| styles.iter().find(|style| style.id == id).cloned();
    let replaced = by_id(&held.id).expect("overridden style kept its id");
    assert_eq!(replaced.name, "Renamed by content");
    assert_eq!(
        by_id("SpAddedStyle").expect("added style").name,
        "Added by content"
    );
    assert_eq!(styles.len(), template_settings.styles.len() + 1);

    let mut front = body();
    front.document = Some(Box::new(DocumentSettings {
        front_matter: Some("title: Review\n".to_owned()),
        ..DocumentSettings::default()
    }));
    let document = made(&h, &owner, "blank-markdown", front)
        .await
        .expect("front");
    let (_, bytes) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("bytes");
    let markdown = String::from_utf8(bytes).expect("utf8");
    assert!(markdown.starts_with("---\ntitle: Review\n"), "{markdown}");
}

#[tokio::test]
async fn invalid_content_and_formats_without_a_rich_model_are_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-refused");
    let twice = RichText::of(vec![
        Block::new("same", BlockKind::Paragraph, "one"),
        Block::new("same", BlockKind::Paragraph, "two"),
    ]);
    let renumbered = made(&h, &owner, "blank-docx", twice)
        .await
        .expect("ids are renumbered");
    let text = text_of(&h, &owner, &renumbered).await;
    assert!(text.contains("one") && text.contains("two"), "{text:?}");

    let mut linked = Block::new("p1", BlockKind::Paragraph, "click");
    linked.marks.push(Mark {
        start: 0,
        end: 5,
        kind: MarkKind::Link {
            href: "javascript:alert(1)".to_owned(),
        },
    });
    let detail = refused_detail(
        made(&h, &owner, "blank-markdown", RichText::of(vec![linked]))
            .await
            .expect_err("scheme"),
    );
    assert!(detail.contains("bad_mark"), "{detail}");

    for template in ["blank-xlsx", "blank-pptx"] {
        let error = made(&h, &owner, template, body())
            .await
            .expect_err("no rich model");
        assert!(
            matches!(error, DocumentsError::Codec(_)),
            "{template}: {error:?}"
        );
    }
}

#[tokio::test]
async fn each_revision_keeps_the_content_it_was_written_with() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-revisions");
    let document = made(&h, &owner, "blank-markdown", body())
        .await
        .expect("create");
    let (first, _) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("first");
    let outcome = h
        .service
        .commit(
            &owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(first.id.clone()),
                body: CommitBody::Ops(vec![EditOp::SetText {
                    text: "# Rewritten\n".to_owned(),
                }]),
                author: api_author(),
                message: None,
                lock_holder: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("commit");
    assert!(matches!(outcome, CommitOutcome::Committed(_)));
    let head = text_of(&h, &owner, &document).await;
    assert!(head.contains("Rewritten") && !head.contains("Quarterly review"));
    let (revision, old) = h
        .service
        .plain_text(&owner, &document, Some(&first.id), &trace())
        .await
        .expect("first revision text");
    assert_eq!(revision.id, first.id);
    assert!(old.contains("Quarterly review") && !old.contains("Rewritten"));
}
