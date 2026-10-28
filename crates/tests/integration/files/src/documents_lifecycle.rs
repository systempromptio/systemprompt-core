use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::jobs::DocumentTrashPurgeJob;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitRequest, CopyRequest, FolderMove, HistoryPage, NewFolderRequest,
    ShareRequest, UpdateRequest,
};
use systemprompt_identifiers::{ConnectionId, DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};
use systemprompt_test_fixtures::test_db_pool;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn document(h: &Harness, owner: &AuthenticatedUser) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "first\nsecond\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

async fn folder(h: &Harness, owner: &AuthenticatedUser) -> DocumentFolderId {
    h.service
        .create_folder(
            owner,
            NewFolderRequest {
                parent: None,
                name: unique("Folder"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}

async fn share(
    h: &Harness,
    owner: &AuthenticatedUser,
    target: GrantTarget,
    subject: (&str, &str),
    level: AccessLevel,
) {
    h.service
        .share(
            owner,
            ShareRequest {
                target,
                subject_kind: subject.0.to_owned(),
                subject_value: subject.1.to_owned(),
                subject_display: None,
                level,
                valid_until: None,
            },
            &trace(),
        )
        .await
        .expect("share");
}

fn set_text(document: &DocumentId, text: &str) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(vec![EditOp::SetText {
            text: text.to_owned(),
        }]),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
    }
}

#[tokio::test]
async fn trash_hides_a_document_and_restore_brings_it_back() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("trash-owner");
    let id = document(&h, &owner).await;
    h.service.trash(&owner, &id, &trace()).await.expect("trash");

    let read = h
        .service
        .get(&owner, &id, &trace())
        .await
        .expect_err("trashed documents are not read");
    assert!(matches!(read, DocumentsError::Trashed { .. }), "{read:?}");
    let edit = h
        .service
        .commit(&owner, set_text(&id, "x\n"), &trace())
        .await
        .expect_err("trashed documents are not edited");
    assert!(matches!(edit, DocumentsError::Trashed { .. }), "{edit:?}");
    let row = h
        .repos
        .documents
        .find_including_trashed(&id)
        .await
        .expect("row")
        .expect("still stored");
    assert_eq!(row.trashed_by.as_ref(), Some(&owner.id));

    let restored = h
        .service
        .restore_document(&owner, &id, &trace())
        .await
        .expect("restore");
    assert!(restored.trashed_at.is_none());
    h.service
        .get(&owner, &id, &trace())
        .await
        .expect("readable again");
}

#[tokio::test]
async fn a_stranger_cannot_learn_a_trashed_document_exists() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("trash-private");
    let stranger = person("trash-stranger");
    let id = document(&h, &owner).await;
    h.service.trash(&owner, &id, &trace()).await.expect("trash");
    let refused = h
        .service
        .get(&stranger, &id, &trace())
        .await
        .expect_err("stranger");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn the_purge_deletes_documents_trashed_past_retention() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("trash-purge");
    let id = document(&h, &owner).await;
    h.service.trash(&owner, &id, &trace()).await.expect("trash");
    let db = test_db_pool().await;
    DocumentTrashPurgeJob::purge(&db, 30)
        .await
        .expect("purge within retention");
    assert!(
        h.repos
            .documents
            .find_including_trashed(&id)
            .await
            .expect("row")
            .is_some(),
        "a document inside retention is kept"
    );
    sqlx::query("UPDATE documents SET deleted_at = now() - interval '31 days' WHERE id = $1::uuid")
        .bind(id.as_str())
        .execute(&*db.write_pool())
        .await
        .expect("age the trash");
    DocumentTrashPurgeJob::purge(&db, 30)
        .await
        .expect("purge past retention");
    assert!(
        h.repos
            .documents
            .find_including_trashed(&id)
            .await
            .expect("row")
            .is_none(),
        "a document past retention is deleted with its revisions"
    );
}

#[tokio::test]
async fn copy_starts_a_new_history_owned_by_the_caller() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("copy-owner");
    let reader = person("copy-reader");
    let id = document(&h, &owner).await;
    let base = h
        .service
        .get(&owner, &id, &trace())
        .await
        .expect("get")
        .head;
    let mut request = set_text(&id, "first\nsecond\nthird\n");
    request.base_revision = base.map(|head| head.id);
    h.service
        .commit(&owner, request, &trace())
        .await
        .expect("second revision");
    share(
        &h,
        &owner,
        GrantTarget::Document(id.clone()),
        ("user", reader.id.as_str()),
        AccessLevel::View,
    )
    .await;
    let (copy, revision) = h
        .service
        .copy(
            &reader,
            CopyRequest {
                source: id.clone(),
                name: None,
                folder: None,
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("copy");
    assert_ne!(copy.id, id);
    assert_eq!(copy.owner_user_id, reader.id);
    assert!(copy.name.contains("(copy)"), "{}", copy.name);
    assert!(revision.parent_revision_id.is_none());
    let history = h
        .service
        .history(
            &reader,
            &copy.id,
            HistoryPage {
                limit: 10,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert_eq!(
        history.len(),
        1,
        "the copy does not carry the source's history"
    );
    let (_, text) = h
        .service
        .read_bytes(&reader, &copy.id, None, &trace())
        .await
        .expect("bytes");
    assert_eq!(String::from_utf8_lossy(&text), "first\nsecond\nthird\n");
}

#[tokio::test]
async fn update_renames_and_moves_and_names_the_owner_when_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("update-owner");
    let reader = person("update-reader");
    let id = document(&h, &owner).await;
    let target = folder(&h, &owner).await;
    let fresh = unique("renamed");
    let updated = h
        .service
        .update(
            &owner,
            &id,
            UpdateRequest {
                name: Some(fresh.clone()),
                folder: FolderMove::Into(target.clone()),
            },
            &trace(),
        )
        .await
        .expect("update");
    assert_eq!(updated.name, format!("{fresh}.txt"));
    assert_eq!(updated.folder_id.as_ref(), Some(&target));

    share(
        &h,
        &owner,
        GrantTarget::Document(id.clone()),
        ("user", reader.id.as_str()),
        AccessLevel::View,
    )
    .await;
    let refused = h
        .service
        .update(
            &reader,
            &id,
            UpdateRequest {
                name: Some("mine".to_owned()),
                folder: FolderMove::Keep,
            },
            &trace(),
        )
        .await
        .expect_err("a viewer cannot rename");
    let DocumentsError::Forbidden { owner: named, .. } = &refused else {
        panic!("expected forbidden, got {refused:?}");
    };
    assert_eq!(named.as_ref(), Some(&owner.id));
    assert!(refused.to_string().contains(owner.id.as_str()), "{refused}");
}

#[tokio::test]
async fn access_lists_owner_direct_and_inherited_grants_with_display_names() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("access-owner");
    let reader = person("access-reader");
    let parent = folder(&h, &owner).await;
    let mut request = create("blank-text", Some(&parent));
    request.ops = vec![EditOp::SetText {
        text: "x\n".to_owned(),
    }];
    let (doc, _) = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create in folder");
    h.service
        .share(
            &owner,
            ShareRequest {
                target: GrantTarget::Document(doc.id.clone()),
                subject_kind: "user".to_owned(),
                subject_value: reader.id.as_str().to_owned(),
                subject_display: Some("Ada Reader".to_owned()),
                level: AccessLevel::Comment,
                valid_until: None,
            },
            &trace(),
        )
        .await
        .expect("share with display");
    share(
        &h,
        &owner,
        GrantTarget::Document(doc.id.clone()),
        ("user", reader.id.as_str()),
        AccessLevel::Comment,
    )
    .await;
    share(
        &h,
        &owner,
        GrantTarget::Folder(parent.clone()),
        ("role", "staff"),
        AccessLevel::View,
    )
    .await;
    let access = h
        .service
        .access_of(&reader, &doc.id, &trace())
        .await
        .expect("access");
    assert_eq!(access.owner, owner.id);
    assert_eq!(access.my_access, AccessLevel::Comment);
    assert_eq!(access.grants.len(), 1);
    assert_eq!(
        access.grants[0].subject_display.as_deref(),
        Some("Ada Reader")
    );
    assert!(
        access
            .inherited
            .iter()
            .any(|grant| grant.subject_kind == "role"
                && grant.subject_value == "staff"
                && grant.subject_display.is_none())
    );
}

#[tokio::test]
async fn ownership_moves_and_the_previous_owner_keeps_edit() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("transfer-from");
    let next = person("transfer-to");
    let id = document(&h, &owner).await;
    let (tx, mut rx) = mpsc::channel(16);
    assert!(
        h.broadcasters
            .agui
            .register(&next.id, &ConnectionId::generate(), tx)
            .await
    );
    let moved = h
        .service
        .transfer_ownership(&owner, &id, &next.id, &trace())
        .await
        .expect("transfer");
    assert_eq!(moved.owner_user_id, next.id);
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(seen.contains("document.ownership_transferred"), "{seen}");
    let view = h
        .service
        .get(&owner, &id, &trace())
        .await
        .expect("previous owner reads");
    assert_eq!(view.my_access, AccessLevel::Edit);
    let access = h
        .service
        .access_of(&owner, &id, &trace())
        .await
        .expect("previous owner reads access");
    let kept = access
        .grants
        .iter()
        .find(|grant| grant.subject_value == owner.id.as_str())
        .expect("previous owner keeps a grant");
    assert_eq!(
        kept.subject_display.as_deref(),
        Some(owner.username.as_str())
    );
    let refused = h
        .service
        .transfer_ownership(&owner, &id, &owner.id, &trace())
        .await
        .expect_err("only the owner transfers");
    assert!(
        matches!(refused, DocumentsError::Forbidden { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn revoke_all_removes_every_grant() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("revoke-owner");
    let reader = person("revoke-reader");
    let id = document(&h, &owner).await;
    let (tx, mut rx) = mpsc::channel(16);
    assert!(
        h.broadcasters
            .agui
            .register(&reader.id, &ConnectionId::generate(), tx)
            .await
    );
    share(
        &h,
        &owner,
        GrantTarget::Document(id.clone()),
        ("user", reader.id.as_str()),
        AccessLevel::View,
    )
    .await;
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(
        seen.contains("document.shared"),
        "the grantee hears the share: {seen}"
    );
    share(
        &h,
        &owner,
        GrantTarget::Document(id.clone()),
        ("role", "staff"),
        AccessLevel::Comment,
    )
    .await;
    let revoked = h
        .service
        .revoke_all(&owner, &id, &trace())
        .await
        .expect("revoke all");
    assert_eq!(revoked, 2);
    let refused = h
        .service
        .get(&reader, &id, &trace())
        .await
        .expect_err("access is gone");
    assert!(
        matches!(refused, DocumentsError::Forbidden { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn repeated_copies_and_restores_take_the_next_free_name() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("names-owner");
    let parent = folder(&h, &owner).await;
    let name = unique("report");
    let make = || {
        let mut request = create("blank-text", Some(&parent));
        request.name = name.clone();
        request
    };
    let source = h
        .service
        .create_from_template(&owner, make(), &trace())
        .await
        .expect("source")
        .0;
    let copy = || CopyRequest {
        source: source.id.clone(),
        name: None,
        folder: None,
        author: api_author(),
    };
    let first = h
        .service
        .copy(&owner, copy(), &trace())
        .await
        .expect("copy")
        .0;
    let second = h
        .service
        .copy(&owner, copy(), &trace())
        .await
        .expect("copy again")
        .0;
    assert_eq!(first.name, format!("{name} (copy).txt"));
    assert_eq!(second.name, format!("{name} (copy 2).txt"));

    let trashed_twin = h
        .service
        .create_from_template(&owner, make(), &trace())
        .await;
    assert!(
        trashed_twin.is_err(),
        "the name is taken while the source is live"
    );
    h.service
        .trash(&owner, &source.id, &trace())
        .await
        .expect("trash source");
    let twin = h
        .service
        .create_from_template(&owner, make(), &trace())
        .await
        .expect("twin takes the name")
        .0;
    h.service
        .trash(&owner, &twin.id, &trace())
        .await
        .expect("trash twin");
    h.service
        .create_from_template(&owner, make(), &trace())
        .await
        .expect("a live document takes the name");
    let first_back = h
        .service
        .restore_document(&owner, &source.id, &trace())
        .await
        .expect("restore source");
    let second_back = h
        .service
        .restore_document(&owner, &twin.id, &trace())
        .await
        .expect("restore twin");
    assert_eq!(first_back.name, format!("{name} (restored).txt"));
    assert_eq!(second_back.name, format!("{name} (restored 2).txt"));
}
