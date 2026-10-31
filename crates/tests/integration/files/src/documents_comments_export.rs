use std::io::{Cursor, Read};

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_files::documents::models::{CommentAnchor, DocumentComment};
use systemprompt_files::documents::service::{CommentRequest, ResolveCommentRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{EditOp, ProjectionNode};
use systemprompt_provider_contracts::documents::CodecError;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, deny, harness, person, trace,
};

async fn docx(h: &Harness, owner: &AuthenticatedUser, lines: &[&str]) -> DocumentId {
    let mut request = create("blank-docx", None);
    let blocks = lines
        .iter()
        .enumerate()
        .map(|(at, text)| Block::new(format!("p{at}"), BlockKind::Paragraph, *text))
        .collect();
    request.content = Some(RichText::of(blocks));
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create docx")
        .0
        .id
}

async fn node(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    text: &str,
) -> ProjectionNode {
    let (_, projection) = h
        .service
        .project(owner, document, None, &trace())
        .await
        .expect("project");
    projection
        .nodes()
        .find(|node| node.text.as_deref() == Some(text))
        .cloned()
        .unwrap_or_else(|| panic!("no paragraph {text}"))
}

async fn comment(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    node: &ProjectionNode,
    body: &str,
) -> DocumentComment {
    h.service
        .create_comment(
            owner,
            CommentRequest {
                range: None,
                mentions: Vec::new(),
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node: node.id.clone(),
                    find: None,
                    nth: None,
                }),
                body: body.to_owned(),
                parent_id: None,
                mention_options: vec![],
            },
            &trace(),
        )
        .await
        .expect("comment")
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut file = archive.by_name(name).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).expect("utf8 part");
    Some(text)
}

fn entry(id: u32, comment: &DocumentComment) -> String {
    format!(
        "<w:comment w:id=\"{id}\" w:author=\"{}\" w:date=\"{}\">",
        comment.author_user_id.as_str(),
        comment
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    )
}

#[tokio::test]
async fn open_comments_export_as_native_word_comments_on_their_paragraphs() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("export-owner");
    let document = docx(&h, &owner, &["First line", "Second line", "Third line"]).await;
    let first = node(&h, &owner, &document, "First line").await;
    let third = node(&h, &owner, &document, "Third line").await;
    let kept = comment(&h, &owner, &document, &first, "Tighten & <shorten>").await;
    let later = comment(&h, &owner, &document, &third, "Cite a source").await;

    let exported = h
        .service
        .export_with_comments(&owner, &document, &trace())
        .await
        .expect("export");

    let record = h
        .service
        .get(&owner, &document, &trace())
        .await
        .expect("get");
    assert_eq!(exported.name, record.document.name);
    let comments = part(&exported.bytes, "word/comments.xml").expect("comments part");
    assert!(
        comments.starts_with(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:comments "
        )
    );
    assert_eq!(comments.matches("<w:comment ").count(), 2);
    let at_kept = comments
        .find(&entry(0, &kept))
        .expect("first comment entry");
    let at_later = comments
        .find(&entry(1, &later))
        .expect("second comment entry");
    assert!(at_kept < at_later);
    assert!(comments.contains("Tighten &amp; &lt;shorten&gt;"));
    assert!(comments.contains("Cite a source"));
    let body = part(&exported.bytes, "word/document.xml").expect("document part");
    for id in 0..2 {
        assert!(body.contains(&format!("<w:commentRangeStart w:id=\"{id}\"/>")));
        assert!(body.contains(&format!(
            "<w:commentRangeEnd w:id=\"{id}\"/><w:r><w:commentReference w:id=\"{id}\"/></w:r>"
        )));
    }
    let start = body
        .find("<w:commentRangeStart w:id=\"0\"/>")
        .expect("start");
    let end = body.find("<w:commentRangeEnd w:id=\"0\"/>").expect("end");
    assert!(body[start..end].contains("First line"));
    assert!(!body[start..end].contains("Second line"));
    let rels = part(&exported.bytes, "word/_rels/document.xml.rels").expect("rels");
    assert!(rels.contains("comments.xml"));
    let types = part(&exported.bytes, "[Content_Types].xml").expect("types");
    assert!(types.contains("/word/comments.xml"));
}

#[tokio::test]
async fn resolved_comments_are_left_out_and_none_open_returns_the_head_unchanged() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("export-resolved");
    let document = docx(&h, &owner, &["Only line"]).await;
    let only = node(&h, &owner, &document, "Only line").await;
    let open = comment(&h, &owner, &document, &only, "Still open").await;
    let closed = comment(&h, &owner, &document, &only, "Already handled").await;
    h.service
        .resolve_comment(
            &owner,
            ResolveCommentRequest {
                document: document.clone(),
                comment: closed.id.clone(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect("resolve");

    let exported = h
        .service
        .export_with_comments(&owner, &document, &trace())
        .await
        .expect("export");
    let comments = part(&exported.bytes, "word/comments.xml").expect("comments part");
    assert_eq!(comments.matches("<w:comment ").count(), 1);
    assert!(comments.contains(&entry(0, &open)));
    assert!(!comments.contains("Already handled"));

    h.service
        .resolve_comment(
            &owner,
            ResolveCommentRequest {
                document: document.clone(),
                comment: open.id.clone(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect("resolve the last");
    let (_, head) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("head bytes");
    let bare = h
        .service
        .export_with_comments(&owner, &document, &trace())
        .await
        .expect("export without comments");
    assert_eq!(bare.bytes, head);
    assert!(part(&bare.bytes, "word/comments.xml").is_none());
}

#[tokio::test]
async fn export_is_refused_for_other_formats_and_for_a_viewer_without_access() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("export-format");
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "plain\n".to_owned(),
    }];
    let text = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create text")
        .0
        .id;
    let refused = h
        .service
        .export_with_comments(&owner, &text, &trace())
        .await
        .expect_err("text cannot carry native comments");
    assert!(matches!(
        refused,
        DocumentsError::Codec(CodecError::UnsupportedOp {
            op: "export_with_comments",
            ..
        })
    ));

    let document = docx(&h, &owner, &["Private line"]).await;
    let denied = harness(deny(), GroupSource::new(&[])).await;
    let stranger = person("export-stranger");
    let refused = denied
        .service
        .export_with_comments(&stranger, &document, &trace())
        .await
        .expect_err("a stranger cannot export");
    assert!(matches!(
        refused,
        DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
    ));
}
