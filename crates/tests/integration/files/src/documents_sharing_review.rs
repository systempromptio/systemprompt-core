use systemprompt_files::documents::models::{DocumentGrant, GrantTarget, NewGrant};
use systemprompt_files::documents::repository::{SubtreeTrash, SubtreeTrashPlan};
use systemprompt_files::documents::service::{
    NewFolderRequest, ShareRequest, TransferRequest, parse_drive_cursor,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DriveChange, DriveChangeKind, EditOp};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace, unique,
};

async fn folder(
    h: &Harness,
    who: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
) -> (DocumentFolderId, String) {
    let created = h
        .service
        .create_folder(
            who,
            NewFolderRequest {
                parent: parent.cloned(),
                name: unique("Folder"),
            },
            &trace(),
        )
        .await
        .expect("folder");
    (created.id, created.name)
}

async fn document_in(
    h: &Harness,
    who: &AuthenticatedUser,
    at: Option<&DocumentFolderId>,
    text: &str,
) -> (DocumentId, String) {
    let mut request = create("blank-text", at);
    request.name = unique("doc");
    request.ops = vec![EditOp::SetText {
        text: format!("{text}\n"),
    }];
    let (document, _) = h
        .service
        .create_from_template(who, request, &trace())
        .await
        .expect("create");
    (document.id, document.name)
}

async fn share(
    h: &Harness,
    owner: &AuthenticatedUser,
    target: GrantTarget,
    to: &AuthenticatedUser,
    level: AccessLevel,
) -> DocumentGrant {
    h.service
        .share(
            owner,
            ShareRequest {
                target,
                subject_kind: "user".to_owned(),
                subject_value: to.id.as_str().to_owned(),
                subject_display: None,
                level,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share")
}

async fn manifest(h: &Harness, who: &AuthenticatedUser) -> String {
    h.service
        .drive_manifest(who, &trace())
        .await
        .expect("manifest")
        .cursor
}

async fn feed_for(
    h: &Harness,
    who: &AuthenticatedUser,
    start: &str,
    document: &DocumentId,
    done: impl Fn(&[DriveChange]) -> bool,
) -> Vec<DriveChange> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let mut cursor = parse_drive_cursor(start).expect("cursor parses");
        let mut seen = Vec::new();
        for _ in 0..100 {
            let page = h
                .service
                .drive_changes(who, Some(cursor), 50, &trace())
                .await
                .expect("changes");
            seen.extend(
                page.changes
                    .into_iter()
                    .filter(|change| &change.document_id == document),
            );
            let next = parse_drive_cursor(&page.next_cursor).expect("next cursor");
            if next == cursor {
                break;
            }
            cursor = next;
        }
        if done(&seen) || std::time::Instant::now() >= deadline {
            return seen;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn a_document_given_away_after_the_partition_is_not_trashed_with_the_folder() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("race-owner");
    let heir = person("race-heir");
    let (root, _) = folder(&h, &owner, None).await;
    let (document, _) = document_in(&h, &owner, Some(&root), "given away").await;
    h.service
        .transfer_ownership(
            &owner,
            &document,
            TransferRequest {
                to: heir.id.clone(),
                to_display: Some("heir".to_owned()),
            },
            &trace(),
        )
        .await
        .expect("transfer");

    let outcome = h
        .repos
        .folders
        .trash_subtree(
            &root,
            &owner.id,
            SubtreeTrashPlan {
                folders: std::slice::from_ref(&root),
                trash: std::slice::from_ref(&document),
                rehome: &[],
            },
        )
        .await
        .expect("trash");
    assert_eq!(outcome, SubtreeTrash::Changed);
    let kept = h
        .repos
        .documents
        .find(&document)
        .await
        .expect("find")
        .expect("the heir's document is not trashed");
    assert_eq!(kept.owner_user_id, heir.id);
    assert!(h.repos.folders.find(&root).await.expect("find").is_some());
}

#[tokio::test]
async fn moving_a_folder_rehomes_a_collaborators_document_and_logs_the_owners_paths() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("move-owner");
    let collaborator = person("move-collaborator");
    let (moving, moving_name) = folder(&h, &owner, None).await;
    let (destination, destination_name) = folder(&h, &owner, None).await;
    let (own, own_name) = document_in(&h, &owner, Some(&moving), "own text").await;
    share(
        &h,
        &owner,
        GrantTarget::Folder(moving.clone()),
        &collaborator,
        AccessLevel::Edit,
    )
    .await;
    let (theirs, theirs_name) = document_in(&h, &collaborator, Some(&moving), "theirs").await;
    let owner_start = manifest(&h, &owner).await;
    let collaborator_start = manifest(&h, &collaborator).await;

    h.service
        .move_folder(&owner, &moving, Some(&destination), &trace())
        .await
        .expect("move");

    let rehomed = h
        .repos
        .documents
        .find(&theirs)
        .await
        .expect("find")
        .expect("the collaborator's document survives");
    assert_eq!(rehomed.folder_id, None, "it stays out of the new parent");
    let carried = h
        .repos
        .documents
        .find(&own)
        .await
        .expect("find")
        .expect("own document");
    assert_eq!(carried.folder_id.as_ref(), Some(&moving));

    let moved_path = format!("{destination_name}/{moving_name}/{own_name}");
    let owner_feed = feed_for(&h, &owner, &owner_start, &own, |seen| {
        seen.iter().any(|c| c.path == moved_path)
    })
    .await;
    assert!(
        owner_feed.iter().any(|c| c.kind
            == DriveChangeKind::Renamed {
                from: format!("{moving_name}/{own_name}")
            }
            && c.path == moved_path),
        "the owner's document under the moved folder is reported renamed: {owner_feed:?}"
    );
    let collaborator_feed = feed_for(&h, &collaborator, &collaborator_start, &theirs, |seen| {
        seen.iter().any(|c| c.path == theirs_name)
    })
    .await;
    assert!(
        collaborator_feed
            .iter()
            .any(|c| matches!(c.kind, DriveChangeKind::Renamed { .. }) && c.path == theirs_name),
        "the collaborator's feed records the move to their root: {collaborator_feed:?}"
    );
}

#[tokio::test]
async fn renaming_a_folder_reports_each_document_beneath_it_renamed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("rename-owner");
    let (parent, parent_name) = folder(&h, &owner, None).await;
    let (child, child_name) = folder(&h, &owner, Some(&parent)).await;
    let (document, name) = document_in(&h, &owner, Some(&child), "nested").await;
    let start = manifest(&h, &owner).await;
    let renamed = unique("Renamed");

    h.service
        .rename_folder(&owner, &parent, &renamed, &trace())
        .await
        .expect("rename");

    let to = format!("{renamed}/{child_name}/{name}");
    let feed: Vec<_> = feed_for(&h, &owner, &start, &document, |seen| {
        seen.iter()
            .any(|c| matches!(c.kind, DriveChangeKind::Renamed { .. }) && c.path == to)
    })
    .await
    .into_iter()
    .skip_while(|c| c.kind == DriveChangeKind::Created)
    .collect();
    assert_eq!(
        feed.iter()
            .map(|c| (c.kind.clone(), c.path.clone()))
            .collect::<Vec<_>>(),
        vec![(
            DriveChangeKind::Renamed {
                from: format!("{parent_name}/{child_name}/{name}")
            },
            to
        )]
    );
}

#[tokio::test]
async fn a_revoked_reader_is_told_to_delete_and_another_reader_is_not() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("revoke-owner");
    let revoked = person("revoke-lost");
    let kept = person("revoke-kept");
    let (document, name) = document_in(&h, &owner, None, "shared").await;
    let lost = share(
        &h,
        &owner,
        GrantTarget::Document(document.clone()),
        &revoked,
        AccessLevel::View,
    )
    .await;
    share(
        &h,
        &owner,
        GrantTarget::Document(document.clone()),
        &kept,
        AccessLevel::View,
    )
    .await;
    let revoked_start = manifest(&h, &revoked).await;
    let kept_start = manifest(&h, &kept).await;

    h.service
        .revoke(
            &owner,
            &GrantTarget::Document(document.clone()),
            lost.id,
            &trace(),
        )
        .await
        .expect("revoke");

    let revoked_feed = feed_for(&h, &revoked, &revoked_start, &document, |seen| {
        !seen.is_empty()
    })
    .await;
    assert_eq!(
        revoked_feed
            .iter()
            .map(|c| (c.kind.clone(), c.path.clone()))
            .collect::<Vec<_>>(),
        vec![(DriveChangeKind::Deleted, name)]
    );
    let kept_feed = feed_for(&h, &kept, &kept_start, &document, |_| false).await;
    assert!(kept_feed.is_empty(), "{kept_feed:?}");
}

#[tokio::test]
async fn a_grant_above_the_sharers_level_is_not_lowered_by_the_write() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("bound-owner");
    let grantee = person("bound-grantee");
    let (document, _) = document_in(&h, &owner, None, "bounded").await;
    share(
        &h,
        &owner,
        GrantTarget::Document(document.clone()),
        &grantee,
        AccessLevel::Share,
    )
    .await;

    let mut tx = h.repos.documents.begin().await.expect("begin");
    let written = h
        .repos
        .grants
        .insert_bounded(
            &mut tx,
            NewGrant {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: grantee.id.as_str().to_owned(),
                subject_display: None,
                access_level: AccessLevel::View,
                granted_by: owner.id.clone(),
                valid_until: None,
                trace_id: TraceId::generate(),
                reason: None,
                via_agent: None,
            },
            AccessLevel::Edit,
        )
        .await
        .expect("bounded insert");
    tx.commit().await.expect("commit");
    assert!(written.is_none(), "{written:?}");
    let level = h
        .repos
        .grants
        .list_for_document(&document)
        .await
        .expect("grants")
        .into_iter()
        .find(|grant| grant.subject_value == grantee.id.as_str())
        .expect("grant")
        .access_level;
    assert_eq!(level, AccessLevel::Share);
}

#[tokio::test]
async fn an_index_write_for_a_revision_that_is_no_longer_the_head_is_ignored() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("index-owner");
    let fresh = unique("freshword").replace('-', "");
    let (document, _) = document_in(&h, &owner, None, &fresh).await;

    h.repos
        .text_index
        .upsert(&document, &DocumentRevisionId::generate(), "stalewordxyz")
        .await
        .expect("upsert");

    let stale = h
        .service
        .search(&owner, "stalewordxyz", 10, &trace())
        .await
        .expect("search");
    assert!(stale.is_empty(), "{stale:?}");
    let current = h
        .service
        .search(&owner, &fresh, 10, &trace())
        .await
        .expect("search");
    assert!(current.iter().any(|result| result.document.id == document));
}

fn changes_of(feed: &[DriveChange]) -> Vec<(DriveChangeKind, String)> {
    feed.iter()
        .map(|c| (c.kind.clone(), c.path.clone()))
        .collect()
}

#[tokio::test]
async fn a_new_grant_delivers_the_document_to_the_grantee_only() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("gain-owner");
    let grantee = person("gain-grantee");
    let bystander = person("gain-bystander");
    let (document, name) = document_in(&h, &owner, None, "delivered").await;
    let grantee_start = manifest(&h, &grantee).await;
    let bystander_start = manifest(&h, &bystander).await;
    let owner_start = manifest(&h, &owner).await;

    share(
        &h,
        &owner,
        GrantTarget::Document(document.clone()),
        &grantee,
        AccessLevel::View,
    )
    .await;

    let head = h
        .repos
        .revisions
        .head(&document)
        .await
        .expect("head")
        .expect("a head revision");
    let feed = feed_for(&h, &grantee, &grantee_start, &document, |seen| {
        !seen.is_empty()
    })
    .await;
    assert_eq!(changes_of(&feed), vec![(DriveChangeKind::Created, name)]);
    assert_eq!(feed[0].revision_id.as_ref(), Some(&head.id));
    assert_eq!(feed[0].content_sha256.as_ref(), Some(&head.content_sha256));
    for (who, start) in [(&bystander, &bystander_start), (&owner, &owner_start)] {
        let unchanged = feed_for(&h, who, start, &document, |_| false).await;
        assert!(unchanged.is_empty(), "{unchanged:?}");
    }
}

#[tokio::test]
async fn an_expired_grant_retracts_the_document_it_delivered() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("expiry-owner");
    let grantee = person("expiry-grantee");
    let (document, name) = document_in(&h, &owner, None, "expiring").await;
    let start = manifest(&h, &grantee).await;
    let mut tx = h.repos.documents.begin().await.expect("begin");
    h.repos
        .grants
        .insert(
            &mut tx,
            NewGrant {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: grantee.id.as_str().to_owned(),
                subject_display: None,
                access_level: AccessLevel::View,
                granted_by: owner.id.clone(),
                valid_until: Some(chrono::Utc::now() + chrono::Duration::seconds(2)),
                trace_id: TraceId::generate(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("grant");
    tx.commit().await.expect("commit");
    let delivered = feed_for(&h, &grantee, &start, &document, |seen| !seen.is_empty()).await;
    assert_eq!(
        changes_of(&delivered),
        vec![(DriveChangeKind::Created, name.clone())]
    );

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    h.repos.grants.sweep_expired(10_000).await.expect("sweep");

    let feed = feed_for(&h, &grantee, &start, &document, |seen| seen.len() >= 2).await;
    assert_eq!(
        changes_of(&feed).last(),
        Some(&(DriveChangeKind::Deleted, name)),
        "{feed:?}"
    );
}

#[tokio::test]
async fn a_grant_revoked_before_the_reader_catches_up_leaves_only_the_delete() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("flash-owner");
    let grantee = person("flash-grantee");
    let (document, name) = document_in(&h, &owner, None, "brief").await;
    let start = manifest(&h, &grantee).await;
    let grant = share(
        &h,
        &owner,
        GrantTarget::Document(document.clone()),
        &grantee,
        AccessLevel::View,
    )
    .await;
    h.service
        .revoke(
            &owner,
            &GrantTarget::Document(document.clone()),
            grant.id,
            &trace(),
        )
        .await
        .expect("revoke");

    let feed = feed_for(&h, &grantee, &start, &document, |seen| !seen.is_empty()).await;
    assert_eq!(changes_of(&feed), vec![(DriveChangeKind::Deleted, name)]);
}
