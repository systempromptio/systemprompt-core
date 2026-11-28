use reqwest::Client;
use reqwest::header::IF_MATCH;
use serde_json::Value;
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::client::{
    ChangeKind, CommitOutcome, DriveListing, FeedChanges, MoveRequest, TrashOutcome, TrashRequest,
};
use systemprompt_models::documents::content_hash;
use systemprompt_scripted_instance::{ModelScript, Route, ScriptedInstance};

use crate::feed::{Feed, committed, part};
use crate::support::{instance, signed_in};

#[tokio::test]
async fn a_commit_lands_in_the_manifest_the_change_feed_and_the_blobs() {
    let feed = Feed::open().await;
    let empty = feed.manifest().await;
    assert_eq!((empty.cursor.as_str(), empty.entries.len()), ("0", 0));

    let first = committed(feed.commit(None, "notes/a.md", None, b"one").await).await;
    let listed = feed.manifest().await;
    assert_eq!(listed.cursor, "1");
    let entry = &listed.entries[0];
    assert_eq!(entry.path, "notes/a.md");
    assert_eq!(entry.head_revision_id, first);
    assert_eq!(entry.content_sha256, content_hash(b"one").as_str());
    assert_eq!(entry.size_bytes, Some(3));

    let page: FeedChanges = feed.changes("0").await.json().await.unwrap();
    assert_eq!(page.next_cursor, "1");
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].kind, ChangeKind::Created);
    assert_eq!(page.changes[0].revision_id.as_ref(), Some(&first));
    let caught_up: FeedChanges = feed.changes("1").await.json().await.unwrap();
    assert!(caught_up.changes.is_empty());

    let blob = feed
        .http
        .get(format!(
            "{}/{}",
            feed.url(ApiPaths::BRIDGE_DRIVE_BLOB),
            entry.content_sha256
        ))
        .bearer_auth(&feed.token)
        .send()
        .await
        .unwrap();
    assert_eq!(blob.bytes().await.unwrap().as_ref(), b"one");

    let routes: Vec<Route> = feed.instance.requests().iter().map(|r| r.route).collect();
    assert!(
        routes
            .iter()
            .filter(|route| **route == Route::Drive)
            .count()
            >= 6
    );
    assert!(feed.instance.unscripted_requests().is_empty());
}

#[tokio::test]
async fn a_commit_off_a_stale_base_is_a_conflict_naming_the_head() {
    let feed = Feed::open().await;
    let first = committed(feed.commit(None, "a.md", None, b"one").await).await;
    let id = feed.manifest().await.entries[0].document_id.clone();
    let second = committed(
        feed.commit(Some(id.clone()), "a.md", Some(first.clone()), b"two")
            .await,
    )
    .await;

    let stale = feed
        .commit(Some(id), "a.md", Some(first.clone()), b"three")
        .await;
    assert_eq!(stale.status().as_u16(), 409);
    let CommitOutcome::Conflict {
        head_revision_id,
        head_content_sha256,
        base_revision_id,
    } = stale.json().await.unwrap()
    else {
        panic!("a stale base is a conflict");
    };
    assert_eq!(head_revision_id, second);
    assert_eq!(
        head_content_sha256.as_deref(),
        Some(content_hash(b"two").as_str())
    );
    assert_eq!(base_revision_id, Some(first));

    let page: FeedChanges = feed.changes("0").await.json().await.unwrap();
    let kinds: Vec<&ChangeKind> = page.changes.iter().map(|change| &change.kind).collect();
    assert_eq!(kinds, [&ChangeKind::Created, &ChangeKind::Updated]);
}

#[tokio::test]
async fn a_commit_without_its_parts_is_refused_and_changes_nothing() {
    let feed = Feed::open().await;
    let mut lone = Vec::new();
    lone.extend_from_slice(part("blob", "application/octet-stream").as_bytes());
    lone.extend_from_slice(b"one\r\n");
    let refused = feed.post_parts(lone).await;
    assert_eq!(refused.status().as_u16(), 422);
    let listed = feed.manifest().await;
    assert_eq!((listed.cursor.as_str(), listed.entries.len()), ("0", 0));
}

#[tokio::test]
async fn a_move_renames_and_a_trash_needs_the_head() {
    let feed = Feed::open().await;
    let head = committed(feed.commit(None, "a.md", None, b"one").await).await;
    let id = feed.manifest().await.entries[0].document_id.clone();

    let moved = feed
        .http
        .post(feed.url(ApiPaths::BRIDGE_DRIVE_MOVE))
        .bearer_auth(&feed.token)
        .json(&MoveRequest {
            document_id: &id,
            path: "b.md",
        })
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status().as_u16(), 204);
    assert_eq!(feed.manifest().await.entries[0].path, "b.md");

    let trash = |base: String| {
        feed.http
            .post(feed.url(ApiPaths::BRIDGE_DRIVE_TRASH))
            .bearer_auth(&feed.token)
            .header(IF_MATCH, base)
            .json(&TrashRequest { document_id: &id })
            .send()
    };
    let stale = trash(format!("\"{}\"", DocumentRevisionId::generate()))
        .await
        .unwrap();
    assert_eq!(stale.status().as_u16(), 409);
    let TrashOutcome::Conflict {
        head_revision_id, ..
    } = stale.json().await.unwrap()
    else {
        panic!("a trash off another revision is a conflict");
    };
    assert_eq!(head_revision_id, head);

    let trashed = trash(format!("\"{head}\"")).await.unwrap();
    assert_eq!(trashed.status().as_u16(), 200);
    assert_eq!(
        trashed.json::<TrashOutcome>().await.unwrap(),
        TrashOutcome::Trashed
    );
    assert!(feed.manifest().await.entries.is_empty());

    let page: FeedChanges = feed.changes("0").await.json().await.unwrap();
    assert_eq!(
        page.changes[1].kind,
        ChangeKind::Renamed {
            from: Some("a.md".to_owned())
        }
    );
    assert_eq!(page.changes[2].kind, ChangeKind::Deleted);
    assert_eq!(page.changes[2].revision_id, None);
}

#[tokio::test]
async fn a_cursor_the_feed_does_not_know_is_a_reset() {
    let feed = Feed::open().await;
    for unknown in ["7", "not-a-cursor"] {
        let gone = feed.changes(unknown).await;
        assert_eq!(gone.status().as_u16(), 410);
        let body: Value = gone.json().await.unwrap();
        assert_eq!(body["error_key"], "cursor_reset");
    }
}

#[tokio::test]
async fn the_feed_takes_only_the_sync_token() {
    let instance = instance(ModelScript::empty()).await;
    let (_client, session) = signed_in(&instance).await;
    let http = Client::new();
    let device = session.device_token.unwrap();
    let url = |path: &str| format!("{}{path}", instance.base_url());

    let with_session = http
        .post(url(ApiPaths::BRIDGE_DRIVE_TOKEN))
        .bearer_auth(session.jwt.expose())
        .send()
        .await
        .unwrap();
    assert_eq!(with_session.status().as_u16(), 401);
    for credential in [session.jwt.expose(), device.expose()] {
        let refused = http
            .get(url(ApiPaths::BRIDGE_DRIVE_MANIFEST))
            .bearer_auth(credential)
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status().as_u16(), 401);
    }
    assert!(
        instance
            .requests()
            .iter()
            .any(|record| record.route == Route::Drive)
    );
}

#[tokio::test]
async fn the_browsing_listing_names_the_documents_the_instance_was_started_with() {
    let instance = ScriptedInstance::builder()
        .document("notes/a.md", b"one".to_vec())
        .document("plan.md", b"two".to_vec())
        .start()
        .await
        .unwrap();
    let (_client, session) = signed_in(&instance).await;
    let http = Client::new();
    let url = |path: &str| format!("{}{path}", instance.base_url());

    let refused = http.get(url(ApiPaths::DOCUMENTS)).send().await.unwrap();
    assert_eq!(refused.status(), 401);

    let listing: DriveListing = http
        .get(url(ApiPaths::DOCUMENTS))
        .query(&[("scope", "mine")])
        .bearer_auth(session.jwt.expose())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut names: Vec<&str> = listing.documents.iter().map(|d| d.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["a.md", "plan.md"]);
    assert!(
        listing
            .documents
            .iter()
            .all(|d| d.owner_user_id == instance.user().user_id)
    );

    let searched: DriveListing = http
        .get(url(ApiPaths::DOCUMENTS))
        .query(&[("query", "PLAN")])
        .bearer_auth(session.jwt.expose())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(searched.documents.len(), 1);

    let folders: DriveListing = http
        .get(url(ApiPaths::DOCUMENT_FOLDERS))
        .bearer_auth(session.jwt.expose())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(folders.folders.is_empty());
    assert!(instance.unscripted_requests().is_empty());
}
