//! The grant audit log, its gating, subject search and validation, folder
//! access with inherited grants, and the share message on `document.shared`.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_events::Broadcaster;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::service::{NewFolderRequest, ShareRequest, TransferRequest};
use systemprompt_identifiers::{ConnectionId, DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp, GrantEventKind};
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GROUP, GroupSource, Harness, RoleDirectory, allow, create, harness, harness_configured, person,
    trace, unique,
};

async fn document(
    h: &Harness,
    owner: &AuthenticatedUser,
    folder: Option<&DocumentFolderId>,
) -> DocumentId {
    let mut request = create("blank-text", folder);
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn share_on(target: GrantTarget, kind: &str, value: &str, level: AccessLevel) -> ShareRequest {
    ShareRequest {
        target,
        subject_kind: kind.to_owned(),
        subject_value: value.to_owned(),
        subject_display: None,
        level,
        valid_until: None,
        message: None,
    }
}

fn doc_share(id: &DocumentId, kind: &str, value: &str, level: AccessLevel) -> ShareRequest {
    share_on(GrantTarget::Document(id.clone()), kind, value, level)
}

async fn history(h: &Harness, owner: &AuthenticatedUser, id: &DocumentId) -> Vec<GrantEventKind> {
    h.service
        .access_of(owner, id, true, &trace())
        .await
        .expect("access")
        .history
        .into_iter()
        .map(|event| event.event)
        .collect()
}

#[tokio::test]
async fn every_grant_change_writes_an_audit_row_newest_first() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("audit-owner");
    let reader = person("audit-reader");
    let next = person("audit-next");
    let id = document(&h, &owner, None).await;
    let granted = h
        .service
        .share(
            &owner,
            doc_share(&id, "user", reader.id.as_str(), AccessLevel::View),
            &trace(),
        )
        .await
        .expect("grant");
    h.service
        .share(
            &owner,
            doc_share(&id, "user", reader.id.as_str(), AccessLevel::Edit),
            &trace(),
        )
        .await
        .expect("change");
    h.service
        .revoke(
            &owner,
            &GrantTarget::Document(id.clone()),
            granted.id,
            &trace(),
        )
        .await
        .expect("revoke");
    h.service
        .share(
            &owner,
            doc_share(&id, "user", reader.id.as_str(), AccessLevel::Comment),
            &trace(),
        )
        .await
        .expect("grant again");
    h.service
        .revoke_all(&owner, &id, &trace())
        .await
        .expect("revoke all");
    let events = history(&h, &owner, &id).await;
    assert_eq!(
        events,
        vec![
            GrantEventKind::RevokedAll,
            GrantEventKind::Granted,
            GrantEventKind::Revoked,
            GrantEventKind::Changed,
            GrantEventKind::Granted,
        ]
    );
    let access = h
        .service
        .access_of(&owner, &id, true, &trace())
        .await
        .expect("access");
    let changed = &access.history[3];
    assert_eq!(changed.old_level, Some(AccessLevel::View));
    assert_eq!(changed.new_level, Some(AccessLevel::Edit));
    assert_eq!(changed.actor_user_id.as_ref(), Some(&owner.id));
    assert!(changed.trace_id.is_some());

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
    let events = history(&h, &next, &id).await;
    assert!(
        events[..2].contains(&GrantEventKind::Transferred),
        "{events:?}"
    );
    assert!(events[..2].contains(&GrantEventKind::Granted), "{events:?}");
}

#[tokio::test]
async fn the_expiry_sweep_records_expired_rows_without_an_actor() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("audit-sweep-owner");
    let reader = person("audit-sweep-reader");
    let id = document(&h, &owner, None).await;
    let mut request = doc_share(&id, "user", reader.id.as_str(), AccessLevel::View);
    request.valid_until = Some(Utc::now() - chrono::Duration::minutes(1));
    h.service
        .share(&owner, request, &trace())
        .await
        .expect("share");
    h.repos.grants.sweep_expired(10_000).await.expect("sweep");
    let access = h
        .service
        .access_of(&owner, &id, true, &trace())
        .await
        .expect("access");
    let expired = access
        .history
        .iter()
        .find(|event| event.event == GrantEventKind::Expired)
        .expect("expired row");
    assert_eq!(expired.actor_user_id, None);
    assert_eq!(expired.old_level, Some(AccessLevel::View));
    assert_eq!(expired.subject_value, reader.id.as_str());
}

#[tokio::test]
async fn history_and_requests_are_withheld_below_share() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("audit-gate-owner");
    let viewer = person("audit-gate-viewer");
    let sharer = person("audit-gate-sharer");
    let id = document(&h, &owner, None).await;
    for (who, level) in [(&viewer, AccessLevel::Edit), (&sharer, AccessLevel::Share)] {
        h.service
            .share(
                &owner,
                doc_share(&id, "user", who.id.as_str(), level),
                &trace(),
            )
            .await
            .expect("share");
    }
    let below = h
        .service
        .access_of(&viewer, &id, true, &trace())
        .await
        .expect("viewer");
    assert!(below.history.is_empty() && below.grants.is_empty());
    let holder = h
        .service
        .access_of(&sharer, &id, true, &trace())
        .await
        .expect("sharer");
    assert_eq!(holder.history.len(), 2);
    let without = h
        .service
        .access_of(&sharer, &id, false, &trace())
        .await
        .expect("sharer");
    assert!(without.history.is_empty());
}

#[tokio::test]
async fn subjects_are_searched_per_kind() {
    let ada = person("search-ada");
    let directory = RoleDirectory::default()
        .with_role("staff", &[])
        .with_role("sales", &[])
        .with_user(&ada.id, "Ada Lovelace");
    let enumerable = GroupSource::new(&[]).enumerable(&["engineering", "eng-ops", "legal"]);
    let h = harness_configured(
        allow(),
        enumerable,
        tmp(),
        directory,
        DocumentsConfig::default(),
    )
    .await;
    let access = h.service.access();
    let users = access
        .search_subjects("user", "ada", 20)
        .await
        .expect("users");
    assert_eq!(users.matches.len(), 1);
    assert_eq!(users.matches[0].value, ada.id.as_str());
    assert_eq!(users.matches[0].display, "Ada Lovelace");
    let roles = access
        .search_subjects("role", "st", 20)
        .await
        .expect("roles");
    assert_eq!(
        roles
            .matches
            .iter()
            .map(|m| m.value.as_str())
            .collect::<Vec<_>>(),
        ["staff"]
    );
    let groups = access
        .search_subjects(GROUP.as_str(), "eng", 1)
        .await
        .expect("groups");
    assert!(groups.searchable);
    assert_eq!(groups.matches.len(), 1);
    let open = harness(allow(), GroupSource::new(&[])).await;
    let unlisted = open
        .service
        .access()
        .search_subjects(GROUP.as_str(), "eng", 20)
        .await
        .expect("unlisted dimension");
    assert!(!unlisted.searchable && unlisted.matches.is_empty());
    assert!(matches!(
        access.search_subjects("planet", "", 20).await,
        Err(DocumentsError::UnknownSubjectKind { .. })
    ));
}

#[tokio::test]
async fn unknown_roles_and_unlisted_dimension_values_are_refused() {
    let directory = RoleDirectory::default().with_role("staff", &[]);
    let groups = GroupSource::new(&[]).enumerable(&["engineering"]);
    let h = harness_configured(
        allow(),
        groups,
        tmp(),
        directory,
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("unknown-owner");
    let id = document(&h, &owner, None).await;
    for (kind, value) in [("role", "nobody-holds-this"), (GROUP.as_str(), "marketing")] {
        let refused = h
            .service
            .share(
                &owner,
                doc_share(&id, kind, value, AccessLevel::View),
                &trace(),
            )
            .await
            .expect_err("unknown subject");
        assert!(
            matches!(refused, DocumentsError::UnknownSubject { .. }),
            "{refused:?}"
        );
    }
    for (kind, value) in [("role", "staff"), (GROUP.as_str(), "engineering")] {
        h.service
            .share(
                &owner,
                doc_share(&id, kind, value, AccessLevel::View),
                &trace(),
            )
            .await
            .expect("known subject");
    }
}

#[tokio::test]
async fn folder_access_names_the_folder_each_inherited_grant_comes_from() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-access-owner");
    let reader = person("folder-access-reader");
    let parent_name = unique("Parent");
    let parent = folder(&h, &owner, None, &parent_name).await;
    let child = folder(&h, &owner, Some(&parent), "Child").await;
    h.service
        .share(
            &owner,
            share_on(
                GrantTarget::Folder(parent.clone()),
                "user",
                reader.id.as_str(),
                AccessLevel::Comment,
            ),
            &trace(),
        )
        .await
        .expect("share parent");
    let access = h
        .service
        .folder_access_of(&owner, &child, true, &trace())
        .await
        .expect("folder access");
    assert_eq!(access.my_access, AccessLevel::Own);
    let inherited = access
        .inherited
        .iter()
        .find(|i| i.grant.subject_value == reader.id.as_str())
        .expect("inherited from parent");
    assert_eq!(inherited.folder_id, parent);
    assert_eq!(inherited.folder_name, parent_name);
    let id = document(&h, &owner, Some(&child)).await;
    let doc_access = h
        .service
        .access_of(&owner, &id, false, &trace())
        .await
        .expect("doc");
    assert!(doc_access.inherited.iter().any(|i| i.folder_id == parent));
    let parent_access = h
        .service
        .folder_access_of(&owner, &parent, true, &trace())
        .await
        .expect("parent access");
    assert!(
        parent_access
            .history
            .iter()
            .any(|e| e.event == GrantEventKind::Granted && e.subject_value == reader.id.as_str())
    );
}

#[tokio::test]
async fn the_share_message_rides_on_document_shared() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("message-owner");
    let reader = person("message-reader");
    let id = document(&h, &owner, None).await;
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&reader.id, &ConnectionId::generate(), tx)
            .await
    );
    let mut request = doc_share(&id, "user", reader.id.as_str(), AccessLevel::View);
    request.message = Some("please review the intro".to_owned());
    h.service
        .share(&owner, request, &trace())
        .await
        .expect("share");
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(
        seen.contains("document.shared") && seen.contains("please review the intro"),
        "{seen}"
    );
    let mut long = doc_share(&id, "user", reader.id.as_str(), AccessLevel::View);
    long.message = Some("x".repeat(1025));
    assert!(matches!(
        h.service.share(&owner, long, &trace()).await,
        Err(DocumentsError::TooLarge { .. })
    ));
}

fn tmp() -> Arc<TempDir> {
    Arc::new(TempDir::new().expect("tempdir"))
}

async fn folder(
    h: &Harness,
    owner: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
    name: &str,
) -> DocumentFolderId {
    h.service
        .create_folder(
            owner,
            NewFolderRequest {
                parent: parent.cloned(),
                name: name.to_owned(),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}
