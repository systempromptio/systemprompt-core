use systemprompt_files::documents::collab::{CollabError, OpsFormat};
use systemprompt_identifiers::{DocumentId, DocumentNodeId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_provider_contracts::documents::{EditOp, NodeTarget};

use crate::documents_collab::Collab;
use crate::documents_collab_live_fixture::{edit, live_collab, open_session};
use crate::documents_service_fixture::{create, person, trace};

async fn from(c: &Collab, who: &AuthenticatedUser, template: &str) -> DocumentId {
    let (document, _) = c
        .documents
        .create_from_template(who, create(template, None), &trace())
        .await
        .expect("create");
    open_session(c, who, &document.id).await;
    document.id
}

fn node(id: &str) -> NodeTarget {
    NodeTarget::Id(DocumentNodeId::new(id))
}

#[test]
fn ops_format_follows_the_document_model() {
    assert_eq!(OpsFormat::of(DocumentFormat::Xlsx), OpsFormat::Sheet);
    assert_eq!(OpsFormat::of(DocumentFormat::Pptx), OpsFormat::Deck);
    assert_eq!(OpsFormat::of(DocumentFormat::Odp), OpsFormat::Deck);
    assert_eq!(OpsFormat::of(DocumentFormat::Html), OpsFormat::Rich);
    assert_eq!(OpsFormat::of(DocumentFormat::Docx), OpsFormat::Rich);
}

#[tokio::test]
async fn a_deck_takes_a_new_slide_and_rewrites_its_text() {
    let c = live_collab().await;
    let owner = person("live-deck");
    let document = from(&c, &owner, "blank-pptx").await;
    let written = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::InsertSlide {
            after: None,
            layout: None,
            title: Some("Quarterly plan".to_owned()),
            body: vec!["first point".to_owned(), "second point".to_owned()],
        }],
    )
    .await
    .expect("insert slide")
    .expect("live");
    assert!(written.seq.is_some());
    edit(
        &c,
        &owner,
        &document,
        vec![EditOp::StrReplace {
            old_str: "Quarterly".to_owned(),
            new_str: "Annual".to_owned(),
        }],
    )
    .await
    .expect("str replace")
    .expect("live");
    let again = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::StrReplace {
            old_str: "Quarterly".to_owned(),
            new_str: "Annual".to_owned(),
        }],
    )
    .await;
    assert!(
        matches!(again, Err(CollabError::InvalidUpdate(_))),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_deck_refuses_an_unknown_slide_and_a_missing_quote() {
    let c = live_collab().await;
    let owner = person("live-deck-refuse");
    let document = from(&c, &owner, "blank-pptx").await;
    for op in [
        EditOp::Delete {
            node: node("slide[99]"),
            expected_hash: None,
        },
        EditOp::InsertSlide {
            after: Some(DocumentNodeId::new("slide[42]")),
            layout: None,
            title: None,
            body: Vec::new(),
        },
        EditOp::StrReplace {
            old_str: "nowhere-in-this-deck".to_owned(),
            new_str: "x".to_owned(),
        },
        EditOp::Delete {
            node: node("bogus"),
            expected_hash: None,
        },
    ] {
        let refused = edit(&c, &owner, &document, vec![op]).await;
        assert!(refused.is_err(), "{refused:?}");
    }
}
