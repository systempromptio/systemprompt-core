//! Folder subtree delete, access-request answer races, concurrent folder
//! moves, stale ownership transfers and folder-grant expiry.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::models::{
    GrantActor, GrantTarget, NewGrant, OwnerTransfer, PresenceUpsert,
};
use systemprompt_files::documents::repository::{SubtreeTrash, SubtreeTrashPlan};
use systemprompt_files::documents::service::{
    AccessDecision, AccessRequestInput, DecideRequest, NewFolderRequest, ShareRequest,
    TransferRequest, parse_drive_cursor,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, AccessRequestStatus, DriveChangeKind, EditOp, PresenceSource, PresenceState,
};
use tempfile::TempDir;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness, harness_configured, person, trace,
    unique,
};

async fn folder(
    h: &Harness,
    who: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
) -> DocumentFolderId {
    h.service
        .create_folder(
            who,
            NewFolderRequest {
                parent: parent.cloned(),
                name: unique("Folder"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}

async fn document_in(
    h: &Harness,
    who: &AuthenticatedUser,
    at: Option<&DocumentFolderId>,
) -> DocumentId {
    let mut request = create("blank-text", at);
    request.name = unique("doc");
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    h.service
        .create_from_template(who, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn share_folder(
    folder: &DocumentFolderId,
    who: &AuthenticatedUser,
    level: AccessLevel,
) -> ShareRequest {
    ShareRequest {
        target: GrantTarget::Folder(folder.clone()),
        subject_kind: "user".to_owned(),
        subject_value: who.id.as_str().to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
        via_agent: None,
    }
}

#[tokio::test]
async fn recursive_delete_rehomes_a_collaborators_document_and_trashes_the_owners() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("subtree-owner");
    let collaborator = person("subtree-collaborator");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let own_doc = document_in(&h, &owner, Some(&sub)).await;
    h.service
        .share(
            &owner,
            share_folder(&root, &collaborator, AccessLevel::Edit),
            &trace(),
        )
        .await
        .expect("share");
    let theirs = document_in(&h, &collaborator, Some(&sub)).await;
    let start = h
        .service
        .drive_manifest(&collaborator, &trace())
        .await
        .expect("manifest")
        .cursor;

    h.service
        .delete_folder(&owner, &root, true, &trace())
        .await
        .expect("the folder owner deletes their tree");

    for id in [&root, &sub] {
        assert!(h.repos.folders.find(id).await.expect("find").is_none());
    }
    assert!(
        h.repos
            .documents
            .find(&own_doc)
            .await
            .expect("find")
            .is_none(),
        "the owner's own document is trashed with the folder"
    );
    let rehomed = h
        .repos
        .documents
        .find(&theirs)
        .await
        .expect("find")
        .expect("the collaborator's document survives");
    assert_eq!(rehomed.folder_id, None, "it lands at its owner's root");
    assert_eq!(rehomed.owner_user_id, collaborator.id);
    h.service
        .plain_text(&collaborator, &theirs, None, &trace())
        .await
        .expect("its owner still reads it");

    let mut cursor = parse_drive_cursor(&start).expect("cursor parses");
    let mut moves = Vec::new();
    for _ in 0..50 {
        let page = h
            .service
            .drive_changes(&collaborator, Some(cursor), 50, &trace())
            .await
            .expect("changes");
        moves.extend(page.changes.into_iter().filter(|c| c.document_id == theirs));
        let next = parse_drive_cursor(&page.next_cursor).expect("next cursor");
        if next == cursor {
            break;
        }
        cursor = next;
    }
    assert!(
        moves
            .iter()
            .any(|c| matches!(c.kind, DriveChangeKind::Renamed { .. }) && c.path == rehomed.name),
        "the feed records the move to the root: {moves:?}"
    );
}

#[tokio::test]
async fn recursive_delete_of_an_owned_subtree_trashes_everything() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("subtree-all");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let a = document_in(&h, &owner, Some(&root)).await;
    let b = document_in(&h, &owner, Some(&sub)).await;
    h.service
        .delete_folder(&owner, &root, true, &trace())
        .await
        .expect("delete");
    for id in [&root, &sub] {
        assert!(h.repos.folders.find(id).await.expect("find").is_none());
    }
    for id in [&a, &b] {
        let row = h
            .repos
            .documents
            .find_including_trashed(id)
            .await
            .expect("find")
            .expect("row");
        assert!(row.trashed_at.is_some());
        assert_eq!(row.trashed_by.as_ref(), Some(&owner.id));
    }
}

async fn requests_on() -> Harness {
    harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        DocumentsConfig {
            access_requests: true,
            ..DocumentsConfig::default()
        },
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_grant_and_decline_land_exactly_one_answer() {
    let h = requests_on().await;
    let owner = person("race-owner");
    let asker = person("race-asker");
    for _ in 0..8 {
        let id = document_in(&h, &owner, None).await;
        let request = h
            .service
            .request_access(
                &asker,
                &id,
                AccessRequestInput {
                    level: AccessLevel::Edit,
                    message: None,
                },
            )
            .await
            .expect("request");
        let decide = |decision| DecideRequest {
            document: id.clone(),
            request: request.id.clone(),
            decision,
        };
        let grant = {
            let service = h.service.clone();
            let owner = owner.clone();
            let decide = decide(AccessDecision::Grant {
                subject_display: None,
            });
            tokio::spawn(async move {
                service
                    .decide_access_request(&owner, decide, &trace())
                    .await
            })
        };
        let decline = {
            let service = h.service.clone();
            let owner = owner.clone();
            let decide = decide(AccessDecision::Decline);
            tokio::spawn(async move {
                service
                    .decide_access_request(&owner, decide, &trace())
                    .await
            })
        };
        let granted = grant.await.expect("grant task");
        let declined = decline.await.expect("decline task");
        assert_ne!(
            granted.is_ok(),
            declined.is_ok(),
            "exactly one answer lands"
        );
        let final_status = h
            .repos
            .access_requests
            .find(&id, &request.id)
            .await
            .expect("find")
            .expect("row")
            .status;
        let asker_grants = h
            .repos
            .grants
            .list_for_document(&id)
            .await
            .expect("grants")
            .into_iter()
            .filter(|g| g.subject_value == asker.id.as_str())
            .count();
        match final_status {
            AccessRequestStatus::Declined => assert_eq!(asker_grants, 0),
            AccessRequestStatus::Granted => assert_eq!(asker_grants, 1),
            other => panic!("unexpected status {other:?}"),
        }
    }
}

#[tokio::test]
async fn crossed_folder_moves_cannot_both_succeed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("crossed-moves");
    for _ in 0..8 {
        let a = folder(&h, &owner, None).await;
        let b = folder(&h, &owner, None).await;
        let (ta, tb) = (trace(), trace());
        let (first, second) = tokio::join!(
            h.service.move_folder(&owner, &a, Some(&b), &ta),
            h.service.move_folder(&owner, &b, Some(&a), &tb),
        );
        assert!(!(first.is_ok() && second.is_ok()), "a cycle was created");
        for id in [&a, &b] {
            let row = h.repos.folders.find(id).await.expect("find").expect("row");
            assert!(!row.ancestor_ids.contains(id));
        }
    }
}

#[tokio::test]
async fn disjoint_folder_moves_both_succeed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("disjoint-moves");
    for _ in 0..4 {
        let (a, b) = (
            folder(&h, &owner, None).await,
            folder(&h, &owner, None).await,
        );
        let (c, d) = (
            folder(&h, &owner, None).await,
            folder(&h, &owner, None).await,
        );
        let (ta, tb) = (trace(), trace());
        let (first, second) = tokio::join!(
            h.service.move_folder(&owner, &a, Some(&c), &ta),
            h.service.move_folder(&owner, &b, Some(&d), &tb),
        );
        first.expect("a under c");
        second.expect("b under d");
        h.service
            .move_folder(&owner, &c, Some(&a), &trace())
            .await
            .expect_err("a folder cannot move under its own child");
    }
}

#[tokio::test]
async fn a_subtree_with_a_document_outside_the_authorized_set_is_not_trashed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("stray-owner");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let listed = document_in(&h, &owner, Some(&root)).await;
    let stray = document_in(&h, &owner, Some(&sub)).await;
    let outcome = h
        .repos
        .folders
        .trash_subtree(
            &root,
            &owner.id,
            SubtreeTrashPlan {
                folders: &[root.clone(), sub.clone()],
                trash: std::slice::from_ref(&listed),
                rehome: &[],
            },
        )
        .await
        .expect("trash");
    assert_eq!(outcome, SubtreeTrash::Changed);
    for id in [&root, &sub] {
        assert!(h.repos.folders.find(id).await.expect("find").is_some());
    }
    for id in [&listed, &stray] {
        assert!(h.repos.documents.find(id).await.expect("find").is_some());
    }

    let missing_folder = h
        .repos
        .folders
        .trash_subtree(
            &root,
            &owner.id,
            SubtreeTrashPlan {
                folders: std::slice::from_ref(&root),
                trash: &[listed.clone(), stray.clone()],
                rehome: &[],
            },
        )
        .await
        .expect("trash");
    assert_eq!(missing_folder, SubtreeTrash::Changed);

    let all = h
        .repos
        .folders
        .trash_subtree(
            &root,
            &owner.id,
            SubtreeTrashPlan {
                folders: &[root.clone(), sub.clone()],
                trash: &[listed, stray],
                rehome: &[],
            },
        )
        .await
        .expect("trash");
    assert_eq!(
        all,
        SubtreeTrash::Trashed {
            documents: 2,
            rehomed: 0,
            folders: 2
        }
    );
}

#[tokio::test]
async fn a_recursive_delete_clears_the_live_state_of_every_document_it_trashed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("teardown-owner");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let doc = document_in(&h, &owner, Some(&sub)).await;
    h.repos
        .presence
        .upsert(
            &PresenceUpsert {
                document_id: doc.clone(),
                user_id: owner.id.clone(),
                source: PresenceSource::Web,
                state: PresenceState::Viewing,
                cursor: None,
            },
            Duration::from_secs(60),
        )
        .await
        .expect("presence");
    h.service
        .delete_folder(&owner, &root, true, &trace())
        .await
        .expect("delete");
    let present = h.repos.presence.users_present(&doc).await.expect("present");
    assert!(present.is_empty(), "{present:?}");
}

#[tokio::test]
async fn a_transfer_from_an_ex_owner_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("transfer-owner");
    let next = person("transfer-next");
    let thief = person("transfer-thief");
    let id = document_in(&h, &owner, None).await;
    h.service
        .transfer_ownership(
            &owner,
            &id,
            TransferRequest {
                to: next.id.clone(),
                to_display: Some("Next".to_owned()),
            },
            &trace(),
        )
        .await
        .expect("transfer");
    let stale = OwnerTransfer {
        document_id: id.clone(),
        from: owner.id.clone(),
        from_display: owner.username.clone(),
        to: thief.id.clone(),
        actor: GrantActor {
            user: owner.id.clone(),
            trace: trace(),
        },
    };
    assert!(
        !h.repos
            .grant_events
            .transfer_owner(&stale)
            .await
            .expect("transfer")
    );
    let row = h
        .repos
        .documents
        .find(&id)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(row.owner_user_id, next.id);
}

#[tokio::test]
async fn an_expired_folder_grant_clears_presence_below_it() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("expiry-owner");
    let guest = person("expiry-guest");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let doc = document_in(&h, &owner, Some(&sub)).await;
    let mut tx = h.repos.documents.begin().await.expect("begin");
    h.repos
        .grants
        .insert(
            &mut tx,
            NewGrant {
                target: GrantTarget::Folder(root.clone()),
                subject_kind: "user".to_owned(),
                subject_value: guest.id.as_str().to_owned(),
                subject_display: None,
                access_level: AccessLevel::View,
                granted_by: owner.id.clone(),
                valid_until: Some(Utc::now() - chrono::Duration::seconds(1)),
                trace_id: trace(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("grant");
    tx.commit().await.expect("commit");
    h.repos
        .presence
        .upsert(
            &PresenceUpsert {
                document_id: doc.clone(),
                user_id: guest.id.clone(),
                source: PresenceSource::Web,
                state: PresenceState::Viewing,
                cursor: None,
            },
            Duration::from_secs(60),
        )
        .await
        .expect("presence");
    h.repos.grants.sweep_expired(10_000).await.expect("sweep");
    let present = h.repos.presence.users_present(&doc).await.expect("present");
    assert!(!present.contains(&guest.id), "{present:?}");
}
