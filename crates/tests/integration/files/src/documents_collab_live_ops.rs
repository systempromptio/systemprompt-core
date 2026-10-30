use std::sync::Arc;

use systemprompt_files::documents::collab::{CollabService, CollabTuning};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::service::LiveEditRequest;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    ContentAs, EditOp, NodeKind, NodeTarget, SetBlockType, SetMarks, TextBlockType,
};

use crate::documents_collab::html_document;
use crate::documents_collab_live_fixture::{
    edit, id_of, insert, live, live_collab, node_with, open_session, seeded, texts,
};
use crate::documents_service_fixture::{person, trace};

#[tokio::test]
async fn a_document_without_an_open_session_is_left_to_the_head() {
    let c = live_collab().await;
    let owner = person("live-ops-none");
    let document = html_document(&c, &owner).await;
    let head = c.documents.live_read(&owner, &document, &trace()).await;
    assert!(head.expect("read").is_none());
    let outcome = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::StrReplace {
            old_str: "x".to_owned(),
            new_str: "y".to_owned(),
        }],
    )
    .await
    .expect("edit");
    assert!(outcome.is_none());
}

#[tokio::test]
async fn a_whole_document_op_is_never_written_live() {
    let c = live_collab().await;
    let owner = person("live-ops-settext");
    let document = html_document(&c, &owner).await;
    open_session(&c, &owner, &document).await;
    let outcome = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::SetText {
            text: "replaced".to_owned(),
        }],
    )
    .await
    .expect("edit");
    assert!(outcome.is_none());
}

#[tokio::test]
async fn no_engine_leaves_every_batch_to_the_head() {
    let c = live_collab().await;
    let bare = CollabService::new(Arc::clone(&c.documents), CollabTuning::default(), None);
    let owner = person("live-ops-bare");
    let document = html_document(&c, &owner).await;
    let outcome = bare
        .edit_live(
            &owner,
            LiveEditRequest {
                document_id: document,
                ops: Vec::new(),
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("edit");
    assert!(outcome.is_none());
}

#[tokio::test]
async fn an_inserted_line_is_logged_and_read_back_live() {
    let c = live_collab().await;
    let owner = person("live-ops-insert");
    let document = seeded(&c, &owner).await;
    let view = live(&c, &owner, &document).await;
    let alpha = node_with(&view, "alpha line").clone();
    let written = edit(
        &c,
        &owner,
        &document,
        vec![insert(
            &alpha,
            NodeKind::Heading { level: 2 },
            "beta",
            ContentAs::Text,
        )],
    )
    .await
    .expect("edit")
    .expect("live");
    assert!(written.seq.is_some());
    assert_eq!(written.applied, 1);
    assert_eq!(written.touched.len(), 1);
    let after = texts(&live(&c, &owner, &document).await);
    let alpha_at = after.iter().position(|t| t == "alpha line").expect("alpha");
    assert_eq!(after.get(alpha_at + 1).map(String::as_str), Some("beta"));
}

#[tokio::test]
async fn replace_and_str_replace_rewrite_the_live_text() {
    let c = live_collab().await;
    let owner = person("live-ops-replace");
    let document = seeded(&c, &owner).await;
    let view = live(&c, &owner, &document).await;
    let alpha = node_with(&view, "alpha line").clone();
    edit(
        &c,
        &owner,
        &document,
        vec![EditOp::ReplaceText {
            node: id_of(&alpha),
            expected_hash: Some(alpha.hash8.clone()),
            text: "gamma line".to_owned(),
            content_as: ContentAs::Text,
        }],
    )
    .await
    .expect("replace")
    .expect("live");
    edit(
        &c,
        &owner,
        &document,
        vec![EditOp::StrReplace {
            old_str: "gamma".to_owned(),
            new_str: "delta".to_owned(),
        }],
    )
    .await
    .expect("str replace")
    .expect("live");
    let after = texts(&live(&c, &owner, &document).await);
    assert!(after.contains(&"delta line".to_owned()), "{after:?}");
    assert!(!after.iter().any(|t| t.contains("alpha")));
}

#[tokio::test]
async fn a_stale_guard_is_refused_and_nothing_is_written() {
    let c = live_collab().await;
    let owner = person("live-ops-stale");
    let document = seeded(&c, &owner).await;
    let view = live(&c, &owner, &document).await;
    let alpha = node_with(&view, "alpha line").clone();
    let refused = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::ReplaceText {
            node: id_of(&alpha),
            expected_hash: Some("00000000".to_owned()),
            text: "never".to_owned(),
            content_as: ContentAs::Text,
        }],
    )
    .await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(texts(&live(&c, &owner, &document).await).contains(&"alpha line".to_owned()));
}

#[tokio::test]
async fn an_unknown_node_is_refused() {
    let c = live_collab().await;
    let owner = person("live-ops-unknown");
    let document = seeded(&c, &owner).await;
    let refused = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::Delete {
            node: NodeTarget::Id(DocumentNodeId::new("n-missing")),
            expected_hash: None,
        }],
    )
    .await;
    assert!(refused.is_err(), "{refused:?}");
}

#[tokio::test]
async fn delete_and_block_type_change_the_live_tree() {
    let c = live_collab().await;
    let owner = person("live-ops-delete");
    let document = seeded(&c, &owner).await;
    let view = live(&c, &owner, &document).await;
    let alpha = node_with(&view, "alpha line").clone();
    edit(
        &c,
        &owner,
        &document,
        vec![insert(&alpha, NodeKind::ListItem, "item", ContentAs::Text)],
    )
    .await
    .expect("insert")
    .expect("live");
    let view = live(&c, &owner, &document).await;
    let item = node_with(&view, "item").clone();
    let alpha = node_with(&view, "alpha line").clone();
    edit(
        &c,
        &owner,
        &document,
        vec![
            EditOp::Delete {
                node: id_of(&item),
                expected_hash: Some(item.hash8.clone()),
            },
            EditOp::SetBlockType(SetBlockType {
                node: id_of(&alpha),
                expected_hash: None,
                block: TextBlockType::Heading { level: 3 },
            }),
            EditOp::SetMarks(SetMarks {
                node: id_of(&alpha),
                expected_hash: None,
                find: Some("alpha".to_owned()),
                nth: None,
                add: Vec::new(),
                remove: vec!["bold".to_owned()],
            }),
        ],
    )
    .await
    .expect("edit")
    .expect("live");
    let after = live(&c, &owner, &document).await;
    assert!(!texts(&after).contains(&"item".to_owned()));
    assert_eq!(
        node_with(&after, "alpha line").kind,
        NodeKind::Heading { level: 3 }
    );
}

#[tokio::test]
async fn markdown_content_falls_back_to_the_model_path() {
    let c = live_collab().await;
    let owner = person("live-ops-markdown");
    let document = seeded(&c, &owner).await;
    let written = edit(
        &c,
        &owner,
        &document,
        vec![EditOp::AppendMarkdown {
            markdown: "**strong** words".to_owned(),
        }],
    )
    .await
    .expect("edit")
    .expect("live");
    assert!(written.seq.is_some());
    assert!(written.touched.is_empty());
    assert!(!written.after.root.is_empty());
    let after = texts(&live(&c, &owner, &document).await);
    assert!(
        after.iter().any(|t| t.contains("strong words")),
        "{after:?}"
    );
    assert!(after.contains(&"alpha line".to_owned()), "{after:?}");
}

#[tokio::test]
async fn a_reader_without_edit_access_is_refused() {
    let c = live_collab().await;
    let owner = person("live-ops-owner");
    let document = seeded(&c, &owner).await;
    let stranger = person("live-ops-stranger");
    let refused = edit(
        &c,
        &stranger,
        &document,
        vec![EditOp::StrReplace {
            old_str: "alpha".to_owned(),
            new_str: "x".to_owned(),
        }],
    )
    .await;
    assert!(refused.is_err(), "{refused:?}");
}
