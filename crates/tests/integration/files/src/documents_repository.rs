use std::time::Duration;

use chrono::Utc;
use systemprompt_files::documents::models::{
    DocumentFolder, EditSessionOpen, GrantTarget, LockHolderKind, LockOutcome, LockRequest,
    NewDocument, NewFolder, NewGrant, NewRevision, PresenceUpsert, RevisionAttribution,
    SubjectPairs, VisibleListing,
};
use systemprompt_files::documents::repository::{
    DocumentRepositories, DocumentRepository, HeadMove,
};
use systemprompt_identifiers::{
    ContentHash, DocumentFolderId, DocumentId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::{
    AccessLevel, ActorKind, DocumentFormat, PresenceSource, PresenceState, RevisionOriginKind,
};
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

async fn repos() -> DocumentRepositories {
    DocumentRepositories::new(&test_db_pool().await)
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn user(prefix: &str) -> UserId {
    UserId::new(unique(prefix))
}

fn hash() -> ContentHash {
    ContentHash::try_new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ))
    .expect("64 hex chars")
}

async fn folder(
    repos: &DocumentRepositories,
    parent: Option<&DocumentFolderId>,
    owner: &UserId,
) -> DocumentFolder {
    repos
        .folders
        .insert(NewFolder {
            parent_id: parent.cloned(),
            name: unique("folder"),
            owner_user_id: owner.clone(),
            created_by: owner.clone(),
        })
        .await
        .expect("insert folder")
}

async fn document(
    repos: &DocumentRepositories,
    folder: Option<&DocumentFolderId>,
    owner: &UserId,
    name: &str,
) -> DocumentId {
    repos
        .documents
        .insert(NewDocument {
            folder_id: folder.cloned(),
            name: name.to_owned(),
            format: DocumentFormat::Markdown,
            mime_type: "text/markdown".to_owned(),
            template_id: None,
            owner_user_id: owner.clone(),
            created_by: owner.clone(),
        })
        .await
        .expect("insert document")
        .id
}

async fn revision(
    repos: &DocumentRepositories,
    document_id: &DocumentId,
    parent: Option<&DocumentRevisionId>,
    author: &UserId,
) -> DocumentRevisionId {
    let content = hash();
    repos
        .blobs
        .upsert(
            &content,
            10,
            &format!("documents/blobs/{}", content.as_str()),
        )
        .await
        .expect("upsert blob");
    repos
        .revisions
        .insert(NewRevision {
            id: DocumentRevisionId::generate(),
            document_id: document_id.clone(),
            parent_revision_id: parent.cloned(),
            content_sha256: content,
            author_user_id: author.clone(),
            actor_kind: ActorKind::User,
            origin_kind: RevisionOriginKind::Api,
            origin_id: None,
            message: None,
            edit_ops: None,
            attribution: RevisionAttribution::default(),
        })
        .await
        .expect("insert revision")
        .id
}

async fn grant_folder(
    repos: &DocumentRepositories,
    folder: &DocumentFolderId,
    kind: &str,
    value: &str,
    level: AccessLevel,
    granted_by: &UserId,
    expired: bool,
) {
    let pool = test_db_pool().await;
    let mut conn = pool
        .write_pool()
        .acquire()
        .await
        .expect("acquire connection");
    repos
        .grants
        .insert(
            &mut conn,
            NewGrant {
                target: GrantTarget::Folder(folder.clone()),
                subject_kind: kind.to_owned(),
                subject_value: value.to_owned(),
                subject_display: None,
                access_level: level,
                granted_by: granted_by.clone(),
                valid_until: expired.then(|| Utc::now() - chrono::Duration::hours(1)),
                trace_id: systemprompt_identifiers::TraceId::generate(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("insert grant");
}

fn listing_by_name(name: &str) -> VisibleListing {
    VisibleListing {
        folder_id: None,
        query: Some(name.to_owned()),
        cursor: None,
        limit: 50,
    }
}

#[tokio::test]
async fn folder_move_rewrites_descendant_ancestors() {
    let repos = repos().await;
    let owner = user("owner");
    let a = folder(&repos, None, &owner).await;
    let b = folder(&repos, Some(&a.id), &owner).await;
    let c = folder(&repos, Some(&b.id), &owner).await;
    assert_eq!(c.ancestor_ids, vec![a.id.clone(), b.id.clone()]);

    let other = folder(&repos, None, &owner).await;
    let moved = repos
        .folders
        .move_to(&b.id, Some(&other.id))
        .await
        .expect("move");
    assert_eq!(moved, 2, "folder and its one descendant are rewritten");
    let c_after = repos.folders.find(&c.id).await.expect("find").expect("c");
    assert_eq!(c_after.ancestor_ids, vec![other.id.clone(), b.id.clone()]);
    let path = repos.folders.path(&c.id).await.expect("path");
    assert_eq!(path, format!("{}/{}/{}", other.name, b.name, c.name));

    let cycle = repos
        .folders
        .move_to(&b.id, Some(&c.id))
        .await
        .expect("cycle move");
    assert_eq!(cycle, 0, "a folder cannot move under its own descendant");

    let root = repos.folders.move_to(&b.id, None).await.expect("to root");
    assert_eq!(root, 2);
    assert_eq!(
        repos.folders.ancestors(&c.id).await.expect("ancestors"),
        vec![b.id.clone()]
    );
}

async fn cas(
    repos: &DocumentRepositories,
    doc: &DocumentId,
    expected: Option<&DocumentRevisionId>,
    new: &DocumentRevisionId,
) -> bool {
    let mut tx = repos.documents.begin().await.expect("begin");
    let moved = DocumentRepository::move_head(
        &mut tx,
        HeadMove {
            document: doc,
            expected,
            new,
            lock_holder: None,
        },
    )
    .await
    .expect("cas");
    tx.commit().await.expect("commit");
    moved
}

#[tokio::test]
async fn move_head_lets_exactly_one_writer_win() {
    let repos = repos().await;
    let owner = user("owner");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let base = revision(&repos, &doc, None, &owner).await;
    assert!(cas(&repos, &doc, None, &base).await);

    let first = revision(&repos, &doc, Some(&base), &owner).await;
    let second = revision(&repos, &doc, Some(&base), &owner).await;
    let (a, b) = tokio::join!(
        cas(&repos, &doc, Some(&base), &first),
        cas(&repos, &doc, Some(&base), &second),
    );
    assert_eq!([a, b].iter().filter(|w| **w).count(), 1);

    let other = document(&repos, None, &owner, &unique("doc")).await;
    let head = repos
        .documents
        .find(&doc)
        .await
        .expect("find")
        .expect("doc");
    let foreign = head.head_revision_id.expect("head");
    assert!(
        !cas(&repos, &other, None, &foreign).await,
        "a revision of another document is never a head"
    );
}

#[tokio::test]
async fn list_visible_honours_folder_grants_owner_and_expiry() {
    let repos = repos().await;
    let owner = user("owner");
    let reader = user("reader");
    let group = unique("sales");
    let root = folder(&repos, None, &owner).await;
    let child = folder(&repos, Some(&root.id), &owner).await;
    let name = unique("plan");
    let doc = document(&repos, Some(&child.id), &owner, &name).await;
    grant_folder(
        &repos,
        &root.id,
        "group",
        &group,
        AccessLevel::View,
        &owner,
        false,
    )
    .await;

    let sales = SubjectPairs::default().with("group", group.clone());
    let (docs, _) = repos
        .documents
        .list_visible(listing_by_name(&name), &sales, &reader)
        .await
        .expect("list");
    assert_eq!(docs.iter().map(|d| &d.id).collect::<Vec<_>>(), vec![&doc]);

    let hr = SubjectPairs::default().with("group", unique("hr"));
    let (docs, _) = repos
        .documents
        .list_visible(listing_by_name(&name), &hr, &reader)
        .await
        .expect("list");
    assert!(docs.is_empty());

    let (docs, _) = repos
        .documents
        .list_visible(listing_by_name(&name), &SubjectPairs::default(), &owner)
        .await
        .expect("list");
    assert_eq!(docs.len(), 1, "owner sees own documents without grants");

    let stale = unique("stale");
    grant_folder(
        &repos,
        &root.id,
        "group",
        &stale,
        AccessLevel::Edit,
        &owner,
        true,
    )
    .await;
    let (docs, _) = repos
        .documents
        .list_visible(
            listing_by_name(&name),
            &SubjectPairs::default().with("group", stale),
            &reader,
        )
        .await
        .expect("list");
    assert!(docs.is_empty(), "expired grants are ignored");
}

#[tokio::test]
async fn effective_level_picks_the_highest_grant() {
    let repos = repos().await;
    let owner = user("owner");
    let root = folder(&repos, None, &owner).await;
    let doc = document(&repos, Some(&root.id), &owner, &unique("doc")).await;
    let group = unique("g");
    let person = unique("u");
    grant_folder(
        &repos,
        &root.id,
        "group",
        &group,
        AccessLevel::Comment,
        &owner,
        false,
    )
    .await;
    let pool = test_db_pool().await;
    let mut conn = pool
        .write_pool()
        .acquire()
        .await
        .expect("acquire connection");
    repos
        .grants
        .insert(
            &mut conn,
            NewGrant {
                target: GrantTarget::Document(doc.clone()),
                subject_kind: "user".to_owned(),
                subject_value: person.clone(),
                subject_display: None,
                access_level: AccessLevel::Share,
                granted_by: owner.clone(),
                valid_until: None,
                trace_id: systemprompt_identifiers::TraceId::generate(),
                reason: None,
                via_agent: None,
            },
        )
        .await
        .expect("grant");
    let subjects = SubjectPairs::default()
        .with("group", group.clone())
        .with("user", person);
    let level = repos
        .grants
        .effective_level(&doc, std::slice::from_ref(&root.id), &subjects)
        .await
        .expect("level");
    assert_eq!(level, Some(AccessLevel::Share));
    let none = repos
        .grants
        .effective_level(
            &doc,
            &[],
            &SubjectPairs::default().with("group", unique("x")),
        )
        .await
        .expect("level");
    assert_eq!(none, None);
    assert_eq!(
        repos
            .grants
            .list_for_document(&doc)
            .await
            .expect("list")
            .len(),
        1
    );
    assert_eq!(
        repos
            .grants
            .list_for_folder(&root.id)
            .await
            .expect("list")
            .len(),
        1
    );
}

#[tokio::test]
async fn lock_acquire_contend_and_expire() {
    let repos = repos().await;
    let owner = user("owner");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let request = |holder: &str, ttl: u64| LockRequest {
        document_id: doc.clone(),
        holder_user_id: owner.clone(),
        holder_kind: LockHolderKind::Bridge,
        holder_id: holder.to_owned(),
        ttl: Duration::from_secs(ttl),
    };
    let first = repos
        .locks
        .acquire(&request("device-a", 0))
        .await
        .expect("acquire");
    assert!(matches!(first, LockOutcome::Acquired(_)));

    let after_expiry = repos
        .locks
        .acquire(&request("device-b", 60))
        .await
        .expect("acquire");
    assert!(
        matches!(after_expiry, LockOutcome::Acquired(_)),
        "expired lease is taken over"
    );

    let contended = repos
        .locks
        .acquire(&request("device-a", 60))
        .await
        .expect("acquire");
    match contended {
        LockOutcome::Held { current } => assert_eq!(current.holder_id, "device-b"),
        LockOutcome::Acquired(_) => panic!("live lease must not be stolen"),
    }
    let renewed = repos
        .locks
        .acquire(&request("device-b", 60))
        .await
        .expect("renew");
    assert!(matches!(renewed, LockOutcome::Acquired(_)));

    assert!(
        !repos
            .locks
            .release(&doc, "device-a")
            .await
            .expect("release")
    );
    assert!(
        repos
            .locks
            .release(&doc, "device-b")
            .await
            .expect("release")
    );
    assert!(repos.locks.current(&doc).await.expect("current").is_none());
}

#[tokio::test]
async fn presence_upsert_and_sweep() {
    let repos = repos().await;
    let owner = user("owner");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let upsert = |user_id: &UserId| PresenceUpsert {
        document_id: doc.clone(),
        user_id: user_id.clone(),
        source: PresenceSource::Web,
        state: PresenceState::Editing,
        cursor: Some(serde_json::json!({"node": "line[1]"})),
    };
    let viewer = user("viewer");
    repos
        .presence
        .upsert(&upsert(&owner), Duration::from_secs(60))
        .await
        .expect("upsert");
    repos
        .presence
        .upsert(&upsert(&owner), Duration::from_secs(60))
        .await
        .expect("upsert");
    repos
        .presence
        .upsert(&upsert(&viewer), Duration::ZERO)
        .await
        .expect("upsert");

    let active = repos.presence.list_active(&doc).await.expect("active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].state, PresenceState::Editing);
    assert_eq!(
        repos.presence.users_present(&doc).await.expect("users"),
        vec![owner]
    );
    assert!(repos.presence.sweep_expired(10_000).await.expect("sweep") >= 1);
}

#[tokio::test]
async fn edit_session_allows_one_open_per_document() {
    let repos = repos().await;
    let owner = user("owner");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let base = revision(&repos, &doc, None, &owner).await;
    let EditSessionOpen::Opened(session) = repos
        .edit_sessions
        .open(&doc, Some(&base))
        .await
        .expect("open")
    else {
        panic!("first open must succeed");
    };
    assert_eq!(
        repos.edit_sessions.open(&doc, None).await.expect("open"),
        EditSessionOpen::AlreadyOpen
    );
    let snapshot = hash();
    repos
        .blobs
        .upsert(&snapshot, 1, "snap")
        .await
        .expect("blob");
    assert!(
        repos
            .edit_sessions
            .set_snapshot(&session.id, &snapshot)
            .await
            .expect("snap")
    );
    assert!(
        repos
            .edit_sessions
            .set_checkpoint(&session.id, &base)
            .await
            .expect("ckpt")
    );
    let open = repos
        .edit_sessions
        .find_open(&doc)
        .await
        .expect("find")
        .expect("open");
    assert_eq!(open.snapshot_sha256, Some(snapshot));
    assert!(repos.edit_sessions.close(&session.id).await.expect("close"));
    assert!(matches!(
        repos.edit_sessions.open(&doc, None).await.expect("reopen"),
        EditSessionOpen::Opened(_)
    ));
}

#[tokio::test]
async fn unreferenced_blobs_exclude_referenced_ones() {
    let repos = repos().await;
    let owner = user("owner");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let rev = revision(&repos, &doc, None, &owner).await;
    let referenced = repos
        .revisions
        .find(&rev)
        .await
        .expect("find")
        .expect("rev")
        .content_sha256;
    let orphan = hash();
    assert!(
        repos
            .blobs
            .upsert(&orphan, 3, "orphan")
            .await
            .expect("blob")
    );
    assert!(
        !repos
            .blobs
            .upsert(&orphan, 3, "orphan")
            .await
            .expect("blob")
    );

    let candidates = repos
        .blobs
        .unreferenced_older_than(0, 100_000)
        .await
        .expect("gc");
    let hashes: Vec<_> = candidates.iter().map(|b| &b.content_sha256).collect();
    assert!(hashes.contains(&&orphan));
    assert!(!hashes.contains(&&referenced));
    assert!(!repos.blobs.delete(&referenced).await.expect("delete"));
    assert!(repos.blobs.delete(&orphan).await.expect("delete"));
    assert!(repos.blobs.find(&orphan).await.expect("find").is_none());
}

#[tokio::test]
async fn text_search_respects_visibility() {
    let repos = repos().await;
    let owner = user("owner");
    let stranger = user("stranger");
    let doc = document(&repos, None, &owner, &unique("doc")).await;
    let rev = revision(&repos, &doc, None, &owner).await;
    let word = format!("zeta{}", Uuid::new_v4().simple());
    repos
        .text_index
        .upsert(&doc, &rev, &format!("quarterly {word} forecast"))
        .await
        .expect("index");

    let hits = repos
        .text_index
        .search(&word, &SubjectPairs::default(), &owner, 10)
        .await
        .expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document_id, doc);
    assert!(hits[0].snippet.contains(&word));

    let hidden = repos
        .text_index
        .search(&word, &SubjectPairs::default(), &stranger, 10)
        .await
        .expect("search");
    assert!(hidden.is_empty());
}

#[tokio::test]
async fn keyset_cursor_round_trips_and_pages_without_duplicates() {
    let repos = repos().await;
    let owner = user("owner");
    let tag = unique("page");
    for i in 0..5 {
        document(&repos, None, &owner, &format!("{tag}-{i}")).await;
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let listing = VisibleListing {
            folder_id: None,
            query: Some(tag.clone()),
            cursor,
            limit: 2,
        };
        let (page, next) = repos
            .documents
            .list_visible(listing, &SubjectPairs::default(), &owner)
            .await
            .expect("page");
        seen.extend(page.into_iter().map(|d| d.id));
        let Some(next) = next else { break };
        let encoded = next.to_string();
        let decoded = encoded.parse().expect("cursor parses");
        assert_eq!(next, decoded);
        cursor = Some(decoded);
    }
    let mut unique_ids = seen.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(seen.len(), 5);
    assert_eq!(unique_ids.len(), 5);
    assert!(
        "not-a-cursor!"
            .parse::<systemprompt_files::documents::models::ListCursor>()
            .is_err()
    );
}
