use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::import_rich;
use systemprompt_files::documents::model::{Block, BlockKind, DocumentSettings, RichText};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::CodecError;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

fn settings() -> DocumentSettings {
    serde_json::from_value(serde_json::json!({
        "numbering": [
            {"id": "SpSteps", "levels": [{"format": "decimal", "text": "%1."}]}
        ],
        "section": {"title_page": true}
    }))
    .expect("settings")
}

async fn make(
    h: &Harness,
    owner: &AuthenticatedUser,
    settings: DocumentSettings,
) -> Result<Vec<u8>, DocumentsError> {
    let mut content = RichText::of(vec![
        Block::new("p1", BlockKind::Paragraph, "Body text"),
        Block::new(
            "l1",
            BlockKind::ListItem {
                level: 0,
                ordered: true,
                numbering: Some("SpSteps".to_owned()),
                checked: None,
            },
            "First step",
        ),
    ]);
    content.document = Some(Box::new(settings));
    let mut request = create("blank-docx", None);
    request.content = Some(content);
    let document = h
        .service
        .create_from_template(owner, request, &trace())
        .await?
        .0
        .id;
    Ok(h.service
        .read_bytes(owner, &document, None, &trace())
        .await?
        .1)
}

#[tokio::test]
async fn content_numbering_and_section_reach_the_written_docx() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-numbering");
    let bytes = make(&h, &owner, settings()).await.expect("create");
    let written = import_rich(&bytes).expect("import").settings();
    assert!(
        written
            .numbering
            .iter()
            .any(|def| def.levels.first().is_some_and(|level| level.text == "%1.")),
        "{:?}",
        written.numbering
    );
    assert!(written.section.expect("section").title_page);
}

#[tokio::test]
async fn a_content_theme_is_refused_when_the_template_has_no_theme_part() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-theme");
    let themed: DocumentSettings = serde_json::from_value(serde_json::json!({
        "theme": {"heading_font": "Georgia"}
    }))
    .expect("settings");
    match make(&h, &owner, themed).await {
        Err(DocumentsError::Codec(CodecError::Refused { detail })) => {
            assert_eq!(detail, "the document has no theme part to restyle");
        },
        other => panic!("expected a refusal, got {other:?}"),
    }
}
