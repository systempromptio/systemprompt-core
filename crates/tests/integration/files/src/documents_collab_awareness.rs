use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::collab::JoinMode;
use systemprompt_files::documents::live::{AWARENESS_SELECTION_MAX, LiveReplay, ResumeToken};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentNodeId};
use systemprompt_models::documents::{
    AwarenessCursor, AwarenessMode, AwarenessState, LiveResyncReason,
};

use crate::documents_collab::{client, collab, edit, html_document, request};
use crate::documents_service_fixture::{person, trace};

fn state(mode: AwarenessMode, color: &str) -> AwarenessState {
    AwarenessState {
        session_id: None,
        color: color.to_owned(),
        mode,
        cursor: None,
        selection: Vec::new(),
        viewport: None,
        agent: None,
    }
}

fn cursor(len: usize) -> AwarenessCursor {
    AwarenessCursor {
        block: DocumentNodeId::new("b1"),
        anchor: "a".repeat(len),
        head: "0".to_owned(),
    }
}

#[tokio::test]
async fn invalid_states_are_refused_before_any_write() {
    let c = collab().await;
    let owner = person("aw-invalid");
    let document = html_document(&c, &owner).await;
    let mut too_many = state(AwarenessMode::Viewing, "#fff");
    too_many.selection = (0..=AWARENESS_SELECTION_MAX).map(|_| cursor(1)).collect();
    let mut oversized = state(AwarenessMode::Viewing, "#fff");
    oversized.cursor = Some(cursor(513));
    for bad in [
        state(AwarenessMode::Viewing, ""),
        state(AwarenessMode::Viewing, &"a".repeat(33)),
        state(AwarenessMode::Viewing, "red;}"),
        too_many,
        oversized,
    ] {
        let refused = c
            .documents
            .publish_awareness(&owner, &document, bad, &trace())
            .await;
        assert!(
            matches!(refused, Err(DocumentsError::InvalidAwareness { .. })),
            "{refused:?}"
        );
    }
    let open = c
        .documents
        .live_open(&owner, &document, &trace())
        .await
        .expect("open");
    assert!(open.awareness.is_empty());
}

#[tokio::test]
async fn a_published_state_is_kept_and_replayed_on_open() {
    let c = collab().await;
    let owner = person("aw-publish");
    let document = html_document(&c, &owner).await;
    let mut posted = state(AwarenessMode::Editing, "rgb(10, 20, 30)");
    posted.cursor = Some(cursor(4));
    let published = c
        .documents
        .publish_awareness(&owner, &document, posted.clone(), &trace())
        .await
        .expect("publish");
    assert_eq!(published.event.user_id, owner.id);
    assert_eq!(published.event.state, posted);
    let open = c
        .documents
        .live_open(&owner, &document, &trace())
        .await
        .expect("open");
    assert_eq!(open.awareness.len(), 1);
    assert_eq!(open.awareness[0].state, posted);
    assert!(open.edit_session.is_none());
    assert!(open.seq.is_none());
}

#[tokio::test]
async fn a_later_state_replaces_the_earlier_one() {
    let c = collab().await;
    let owner = person("aw-merge");
    let document = html_document(&c, &owner).await;
    for color in ["#111", "#222"] {
        c.documents
            .publish_awareness(
                &owner,
                &document,
                state(AwarenessMode::Viewing, color),
                &trace(),
            )
            .await
            .expect("publish");
    }
    let open = c
        .documents
        .live_open(&owner, &document, &trace())
        .await
        .expect("open");
    assert_eq!(open.awareness.len(), 1);
    assert_eq!(open.awareness[0].state.color, "#222");
}

#[test]
fn resume_tokens_round_trip_and_reject_garbage() {
    let session = DocumentEditSessionId::generate();
    let token = ResumeToken {
        session: session.clone(),
        seq: 7,
    };
    assert_eq!(ResumeToken::parse(&token.to_string()), Some(token));
    assert_eq!(ResumeToken::parse("no-separator"), None);
    assert_eq!(ResumeToken::parse(&format!("{session}:x")), None);
}

#[tokio::test]
async fn live_allowed_follows_view_access() {
    let c = collab().await;
    let owner = person("aw-allowed");
    let document = html_document(&c, &owner).await;
    assert!(
        c.documents
            .live_allowed(&owner, &document, &trace())
            .await
            .expect("allowed")
    );
}

#[tokio::test]
async fn replay_answers_updates_after_the_token_or_a_resync() {
    let c = collab().await;
    let owner = person("aw-replay");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    for text in ["<p>one</p>", "<p>two</p>"] {
        let update = edit(&doc, text);
        c.collab
            .apply(&owner, request(&document, &joined, &doc, update), &trace())
            .await
            .expect("apply");
    }
    let open = c
        .documents
        .live_open(&owner, &document, &trace())
        .await
        .expect("open");
    assert_eq!(open.edit_session.as_ref(), Some(&joined.session_id));
    let head = open.seq.expect("head seq");
    let from = ResumeToken {
        session: joined.session_id.clone(),
        seq: head - 1,
    };
    let LiveReplay::Updates(updates) = c.documents.live_replay(&open, &from).await.expect("replay")
    else {
        panic!("expected updates");
    };
    assert_eq!(
        updates.iter().map(|u| u.seq).collect::<Vec<_>>(),
        vec![head]
    );
    assert_eq!(updates[0].author_id, owner.id);
    let other = ResumeToken {
        session: DocumentEditSessionId::generate(),
        seq: 0,
    };
    assert!(matches!(
        c.documents
            .live_replay(&open, &other)
            .await
            .expect("replay"),
        LiveReplay::Resync(LiveResyncReason::SessionChanged)
    ));
}
