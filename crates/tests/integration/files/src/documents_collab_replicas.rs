use std::sync::Arc;
use std::time::Duration;

use systemprompt_engine_loro::rich;
use systemprompt_files::documents::agents::{DocumentActor, act_as};
use systemprompt_files::documents::collab::{Checkpoint, CollabService, CollabTuning, JoinMode};
use systemprompt_files::documents::model::{EditLevel, Media};
use systemprompt_files::documents::repository::{
    CheckpointClaim, CheckpointLease, CollabAppend, UpdateAuthor,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_collab::{Collab, client, collab, edit, html_document, loro, request};
use crate::documents_service_fixture::{api_author, person, trace};

fn replica(c: &Collab, tuning: CollabTuning) -> Arc<CollabService> {
    Arc::new(CollabService::new(Arc::clone(&c.documents), tuning, loro()))
}

fn eager(c: &Collab) -> Arc<CollabService> {
    replica(
        c,
        CollabTuning {
            snapshot_after: Duration::ZERO,
            snapshot_after_updates: 10_000,
            checkpoint_after: Duration::ZERO,
            tick: Duration::from_millis(10),
            evict_after: Duration::ZERO,
        },
    )
}

async fn head_html(c: &Collab, owner: &AuthenticatedUser, document: &DocumentId) -> String {
    let (_, bytes) = c
        .documents
        .read_bytes(owner, document, None, &trace())
        .await
        .expect("read");
    String::from_utf8(bytes).expect("utf8")
}

#[tokio::test]
async fn a_cached_replica_behind_a_pruning_snapshot_reopens_it() {
    let c = collab().await;
    let pruning = replica(
        &c,
        CollabTuning {
            snapshot_after_updates: 1,
            ..CollabTuning::default()
        },
    );
    let owner = person("replica-prune");
    let document = html_document(&c, &owner).await;
    let first = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join first replica");
    let doc = client(&first);
    let update = edit(&doc, "<p>first</p>");
    c.collab
        .apply(&owner, request(&document, &first, &doc, update), &trace())
        .await
        .expect("apply on the first replica");

    let second = pruning
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join pruning replica");
    let other = client(&second);
    for text in ["<p>second</p>", "<p>third</p>", "<p>fourth</p>"] {
        let update = edit(&other, text);
        pruning
            .apply(
                &owner,
                request(&document, &second, &other, update),
                &trace(),
            )
            .await
            .expect("apply on the pruning replica");
    }
    let snapshot_seq =
        c.h.repos
            .collab
            .snapshot_seq(&first.session_id)
            .await
            .expect("snapshot seq")
            .expect("session");
    assert_eq!(
        snapshot_seq, 4,
        "the pruning replica snapshotted every update"
    );
    assert!(
        c.h.repos
            .collab
            .backlog(&first.session_id, 1)
            .await
            .expect("backlog")
            .snapshot
            .is_some_and(|stored| stored.seq == 4),
        "a replica behind the snapshot is handed the snapshot"
    );

    let rejoined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("rejoin the cached replica");
    assert_eq!(rejoined.seq, 4);
    let model = rich::read(&client(&rejoined)).expect("read");
    for text in ["first", "second", "third", "fourth"] {
        assert!(
            model.blocks.iter().any(|block| block.text.contains(text)),
            "{text} missing from the cached replica"
        );
    }
    let fresh = pruning
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("rejoin the pruning replica");
    assert_eq!(
        loro::VersionVector::decode(&rejoined.version).expect("cached version"),
        loro::VersionVector::decode(&fresh.version).expect("fresh version")
    );
}

#[tokio::test]
async fn concurrent_first_joins_on_two_replicas_open_one_stored_seed() {
    let c = collab().await;
    let other = replica(&c, CollabTuning::default());
    let owner = person("replica-seed");
    let document = html_document(&c, &owner).await;
    let (left_trace, right_trace) = (trace(), trace());
    let (left, right) = tokio::join!(
        c.collab
            .join(&owner, &document, JoinMode::Edit, &left_trace),
        other.join(&owner, &document, JoinMode::Edit, &right_trace),
    );
    let left = left.expect("join left");
    let right = right.expect("join right");
    assert_eq!(left.session_id, right.session_id);
    assert_eq!(
        loro::VersionVector::decode(&left.version).expect("left version"),
        loro::VersionVector::decode(&right.version).expect("right version")
    );
    let stored =
        c.h.repos
            .collab
            .stored_snapshot(&left.session_id)
            .await
            .expect("stored snapshot")
            .expect("the seed is stored");
    assert_eq!(stored.seq, 0);

    let late = replica(&c, CollabTuning::default())
        .join(&owner, &document, JoinMode::View, &trace())
        .await
        .expect("join a third replica");
    assert_eq!(
        loro::VersionVector::decode(&late.version).expect("late version"),
        loro::VersionVector::decode(&left.version).expect("left version")
    );
    assert_eq!(
        c.h.repos
            .collab
            .stored_snapshot(&left.session_id)
            .await
            .expect("stored snapshot")
            .expect("seed")
            .hash,
        stored.hash
    );
}

#[tokio::test]
async fn a_checkpoint_on_a_stale_replica_builds_on_the_latest_checkpoint() {
    let c = collab().await;
    let first = eager(&c);
    let second = eager(&c);
    let owner = person("replica-base");
    let document = html_document(&c, &owner).await;
    let joined = first
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join first");
    second
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("cache the session on the second replica");
    let doc = client(&joined);
    let update = edit(&doc, "<p>one</p>");
    first
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply on first");
    first.tick().await;
    let after_first =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("open")
            .checkpoint_revision_id
            .expect("first checkpoint");

    let update = edit(&doc, "<p>two</p>");
    second
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply on second");
    second.tick().await;
    let after_second =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("open")
            .checkpoint_revision_id
            .expect("second checkpoint");
    assert_ne!(after_first, after_second);
    let revision =
        c.h.repos
            .revisions
            .find(&after_second)
            .await
            .expect("find revision")
            .expect("revision");
    assert_eq!(revision.parent_revision_id, Some(after_first));
    assert!(
        c.h.repos
            .collab
            .list_after(&joined.session_id, 2)
            .await
            .expect("rows")
            .is_empty(),
        "a checkpoint on the current base merges nothing back into the live model"
    );
    let html = head_html(&c, &owner, &document).await;
    assert!(
        html.contains("<p>one</p>") && html.contains("<p>two</p>"),
        "{html}"
    );
}

#[tokio::test]
async fn a_session_closes_only_when_nothing_was_logged_after_the_settled_body() {
    let c = collab().await;
    let owner = person("replica-close");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let sessions = &c.h.repos.edit_sessions;
    let lease = CheckpointLease::generate();
    assert!(matches!(
        sessions
            .claim_checkpoint(&joined.session_id, &lease, Duration::from_secs(60))
            .await
            .expect("claim"),
        CheckpointClaim::Claimed(_)
    ));
    let seq =
        c.h.repos
            .collab
            .append(
                &joined.session_id,
                b"racing update",
                UpdateAuthor {
                    user: &owner.id,
                    actor: &DocumentActor::Human,
                    level: EditLevel::Edit,
                },
            )
            .await
            .expect("append")
            .expect("open session");
    assert_eq!(seq, 1);
    assert!(
        !sessions
            .close_settled(&joined.session_id, 0, &lease)
            .await
            .expect("close on a stale settle"),
        "an update logged after the settled body keeps the session open"
    );
    assert!(
        !sessions
            .close_settled(&joined.session_id, 1, &CheckpointLease::generate())
            .await
            .expect("close without the lease")
    );
    assert!(
        sessions
            .close_settled(&joined.session_id, 1, &lease)
            .await
            .expect("close")
    );
    assert!(sessions.find_open(&document).await.expect("find").is_none());
}

#[tokio::test]
async fn leaving_on_one_replica_commits_updates_logged_on_another() {
    let c = collab().await;
    let other = replica(&c, CollabTuning::default());
    let owner = person("replica-leave");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>here</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply here");
    other
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join other");
    let update = edit(&doc, "<p>there</p>");
    other
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply there");
    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("leave");
    assert!(left.closed);
    let html = head_html(&c, &owner, &document).await;
    assert!(
        html.contains("<p>here</p>") && html.contains("<p>there</p>"),
        "{html}"
    );
}

#[tokio::test]
async fn the_reaper_settles_and_closes_an_abandoned_session() {
    let c = collab().await;
    let owner = person("replica-reap");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>abandoned</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    c.h.repos
        .collab
        .remove_web_presence(&document, &owner.id)
        .await
        .expect("the client vanishes");
    let pool = test_db_pool().await;
    sqlx::query(
        "UPDATE document_edit_sessions SET updated_at = now() - interval '30 years'
         WHERE id = $1::uuid",
    )
    .bind(joined.session_id.as_str())
    .execute(&*pool.write_pool())
    .await
    .expect("age the session");

    let reaper = replica(&c, CollabTuning::default());
    let reaped = reaper
        .reap_idle(Duration::from_secs(20 * 365 * 24 * 60 * 60), 50)
        .await
        .expect("reap");
    assert!(reaped.closed >= 1, "{reaped:?}");
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_none()
    );
    assert!(
        head_html(&c, &owner, &document)
            .await
            .contains("<p>abandoned</p>")
    );
}

async fn age_and_forget(c: &Collab, document: &DocumentId, users: &[&AuthenticatedUser]) {
    for user in users {
        c.h.repos
            .collab
            .remove_web_presence(document, &user.id)
            .await
            .expect("the client vanishes");
    }
    let pool = test_db_pool().await;
    sqlx::query(
        "UPDATE document_edit_sessions SET updated_at = now() - interval '30 years'
         WHERE document_id = $1::uuid AND closed_at IS NULL",
    )
    .bind(document.as_str())
    .execute(&*pool.write_pool())
    .await
    .expect("age the session");
}

#[tokio::test]
async fn every_replica_logs_the_actor_and_level_each_update_ran_as() {
    let c = collab().await;
    let other = replica(&c, CollabTuning::default());
    let owner = person("replica-actor");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>by the agent</p>");
    act_as(
        DocumentActor::agent("replica-agent", false),
        c.collab
            .apply(&owner, request(&document, &joined, &doc, update), &trace()),
    )
    .await
    .expect("apply as an agent");
    other
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join other");
    let update = edit(&doc, "<p>by the human</p>");
    act_as(
        DocumentActor::Human,
        other.apply(&owner, request(&document, &joined, &doc, update), &trace()),
    )
    .await
    .expect("apply as the human on the other replica");

    let authors =
        c.h.repos
            .collab
            .list_authors_since_checkpoint(&joined.session_id)
            .await
            .expect("authors");
    assert_eq!(authors.len(), 2, "{authors:?}");
    assert!(
        authors[0]
            .actor
            .as_ref()
            .is_some_and(DocumentActor::is_agent)
    );
    assert_eq!(authors[1].actor, Some(DocumentActor::Human));
    assert!(
        authors
            .iter()
            .all(|author| author.level == Some(EditLevel::Edit) && author.user == owner.id)
    );
}

#[tokio::test]
async fn a_session_whose_last_writer_only_commented_still_settles() {
    let c = collab().await;
    let owner = person("replica-commenter-owner");
    let commenter = person("replica-commenter");
    let document = html_document(&c, &owner).await;
    crate::documents_proposals::grant(
        &c.h,
        &owner,
        &document,
        &commenter,
        systemprompt_models::documents::AccessLevel::Comment,
    )
    .await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("owner joins");
    let doc = client(&joined);
    let update = edit(&doc, "<p>owner text</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("owner edits");

    let commenting = c
        .collab
        .join(&commenter, &document, JoinMode::Comment, &trace())
        .await
        .expect("commenter joins");
    let notes = client(&commenting);
    let update = crate::documents_collab::change(&notes, |model| {
        let block = model
            .blocks
            .iter_mut()
            .find(|block| block.text.contains("owner text"))
            .expect("owner block");
        block
            .marks
            .push(systemprompt_files::documents::model::Mark {
                start: 0,
                end: 5,
                kind: systemprompt_files::documents::model::MarkKind::Comment {
                    thread: "t-commenter".to_owned(),
                },
            });
    });
    c.collab
        .apply(
            &commenter,
            request(&document, &commenting, &notes, update),
            &trace(),
        )
        .await
        .expect("commenter comments");

    c.collab
        .leave(&commenter, &document, &commenting.session_id, &trace())
        .await
        .expect("commenter leaves");
    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("the owner's leave settles although the last writer only commented");
    assert!(left.closed, "{left:?}");
    let head = head_html(&c, &owner, &document).await;
    assert!(
        head.contains(r#"<span data-comment="t-commenter">owner</span> text"#),
        "the owner's text and the commenter's mark both land: {head}"
    );
}

#[tokio::test]
async fn a_followed_head_is_nobodys_authorship_and_a_commenter_still_settles() {
    let c = collab().await;
    let service = eager(&c);
    let owner = person("follow-owner");
    let commenter = person("follow-commenter");
    let document = html_document(&c, &owner).await;
    crate::documents_proposals::grant(
        &c.h,
        &owner,
        &document,
        &commenter,
        systemprompt_models::documents::AccessLevel::Comment,
    )
    .await;
    let commenting = service
        .join(&commenter, &document, JoinMode::Comment, &trace())
        .await
        .expect("commenter joins");

    let (head, bytes) = c
        .documents
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("read");
    let outside =
        String::from_utf8(bytes)
            .expect("utf8")
            .replacen("<body>", "<body><h1>Outside</h1>", 1);
    c.documents
        .commit(
            &owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(head.id),
                body: CommitBody::Bytes(outside.into_bytes()),
                author: api_author(),
                message: None,
                lock_holder_id: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("outside commit");
    service.tick().await;

    let authors =
        c.h.repos
            .collab
            .list_authors_since_checkpoint(&commenting.session_id)
            .await
            .expect("authors");
    assert!(
        !authors.is_empty()
            && authors
                .iter()
                .all(|author| author.actor == Some(DocumentActor::system("collab-write-back"))),
        "the followed head is logged as the write-back, not as the commenter: {authors:?}"
    );

    let commenting = service
        .join(&commenter, &document, JoinMode::Comment, &trace())
        .await
        .expect("commenter rejoins the followed session");
    let notes = client(&commenting);
    let update = crate::documents_collab::change(&notes, |model| {
        let block = model
            .blocks
            .iter_mut()
            .find(|block| block.text.contains("Outside"))
            .expect("followed heading");
        block
            .marks
            .push(systemprompt_files::documents::model::Mark {
                start: 0,
                end: 7,
                kind: systemprompt_files::documents::model::MarkKind::Comment {
                    thread: "t-follow".to_owned(),
                },
            });
    });
    service
        .apply(
            &commenter,
            request(&document, &commenting, &notes, update),
            &trace(),
        )
        .await
        .expect("commenter comments");
    let left = service
        .leave(&commenter, &document, &commenting.session_id, &trace())
        .await
        .expect("the commenter's leave settles");
    assert!(left.closed, "{left:?}");
    assert!(
        matches!(
            left.checkpoint,
            Checkpoint::Conflict {
                proposal: Some(_),
                ..
            }
        ),
        "the comment-level body is kept as a proposal: {left:?}"
    );
}

#[tokio::test]
async fn the_reaper_commits_as_the_last_edit_author_not_the_owner() {
    let c = collab().await;
    let owner = person("replica-reap-owner");
    let editor = person("replica-reap-editor");
    let document = html_document(&c, &owner).await;
    crate::documents_proposals::grant(
        &c.h,
        &owner,
        &document,
        &editor,
        systemprompt_models::documents::AccessLevel::Edit,
    )
    .await;
    let joined = c
        .collab
        .join(&editor, &document, JoinMode::Edit, &trace())
        .await
        .expect("editor joins");
    let doc = client(&joined);
    let update = edit(&doc, "<p>the editor's words</p>");
    c.collab
        .apply(&editor, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("editor edits");
    age_and_forget(&c, &document, &[&editor]).await;

    let reaped = replica(&c, CollabTuning::default())
        .reap_idle(Duration::from_secs(20 * 365 * 24 * 60 * 60), 50)
        .await
        .expect("reap");
    assert!(reaped.closed >= 1, "{reaped:?}");
    let head =
        c.h.repos
            .documents
            .find(&document)
            .await
            .expect("find")
            .expect("document")
            .head_revision_id
            .expect("head");
    let revision =
        c.h.repos
            .revisions
            .find(&head)
            .await
            .expect("find revision")
            .expect("revision");
    assert_eq!(revision.author_user_id, editor.id);
    assert_ne!(revision.author_user_id, owner.id);
    assert_eq!(
        revision.actor_kind,
        systemprompt_models::documents::ActorKind::System
    );
    assert_eq!(revision.attribution.on_behalf_of, Some(editor.id.clone()));
}

#[tokio::test]
async fn an_append_validated_before_a_prune_is_refused_as_behind() {
    let c = collab().await;
    let owner = person("replica-cas");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    for text in ["<p>a</p>", "<p>b</p>"] {
        let update = edit(&doc, text);
        c.collab
            .apply(&owner, request(&document, &joined, &doc, update), &trace())
            .await
            .expect("apply");
    }
    let pool = test_db_pool().await;
    sqlx::query("DELETE FROM document_collab_updates WHERE session_id = $1::uuid")
        .bind(joined.session_id.as_str())
        .execute(&*pool.write_pool())
        .await
        .expect("prune every logged update");
    let author = UpdateAuthor {
        user: &owner.id,
        actor: &DocumentActor::Human,
        level: EditLevel::Edit,
    };
    let stale =
        c.h.repos
            .collab
            .append_on(&joined.session_id, 0, b"stale", author)
            .await
            .expect("append on a stale log");
    assert_eq!(stale, CollabAppend::Behind);
    let current =
        c.h.repos
            .collab
            .append_on(&joined.session_id, 2, b"current", author)
            .await
            .expect("append on the current log");
    assert_eq!(current, CollabAppend::Appended(3));
}

#[tokio::test]
async fn leaving_a_session_that_is_already_closed_answers_closed() {
    let c = collab().await;
    let owner = person("replica-closed-leave");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    assert!(
        c.h.repos
            .edit_sessions
            .close(&joined.session_id)
            .await
            .expect("close elsewhere")
    );
    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("leave a closed session");
    assert!(left.closed);
}

#[tokio::test]
async fn an_unreadable_session_is_closed_and_the_next_join_reseeds_it() {
    let c = collab().await;
    let owner = person("replica-reseed");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>before the corruption</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    c.h.repos
        .collab
        .append(
            &joined.session_id,
            b"not an engine update",
            UpdateAuthor {
                user: &owner.id,
                actor: &DocumentActor::Human,
                level: EditLevel::Edit,
            },
        )
        .await
        .expect("log a corrupt row")
        .expect("open");

    let rejoined = replica(&c, CollabTuning::default())
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("the join reseeds instead of failing");
    assert_ne!(rejoined.session_id, joined.session_id);
    let open =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("open");
    assert_eq!(open.id, rejoined.session_id);
}
