//! Drive change feed ordering: changes are delivered in landing order, a
//! transaction that began earlier but lands later is still delivered, an
//! accepted proposal appears, a transaction that lands nothing does not
//! hold the feed back, and a cursor of the old shape asks for a reset.

use std::collections::HashSet;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::{
    FolderMove, ReviewRequest, UpdateRequest, parse_drive_cursor,
};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DriveChange, DriveChangeKind};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_proposals::{grant, memo, outline, propose, rewrite};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn manifest_cursor(h: &Harness, who: &AuthenticatedUser) -> String {
    h.service
        .drive_manifest(who, &trace())
        .await
        .expect("manifest")
        .cursor
}

async fn drain(h: &Harness, who: &AuthenticatedUser, start: &str) -> (Vec<DriveChange>, String) {
    let mut cursor = parse_drive_cursor(start).expect("cursor parses");
    let mut seen = Vec::new();
    for _ in 0..500 {
        let page = h
            .service
            .drive_changes(who, Some(cursor), 3, &trace())
            .await
            .expect("changes");
        seen.extend(page.changes);
        let next = parse_drive_cursor(&page.next_cursor).expect("next cursor");
        assert!(next >= cursor, "the feed never moves backwards");
        if next == cursor {
            break;
        }
        cursor = next;
    }
    (seen, cursor.to_string())
}

async fn drain_settled(
    h: &Harness,
    who: &AuthenticatedUser,
    start: &str,
    done: impl Fn(&[DriveChange]) -> bool,
) -> (Vec<DriveChange>, String) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let (changes, cursor) = drain(h, who, start).await;
        if done(&changes) || std::time::Instant::now() >= deadline {
            return (changes, cursor);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn only(changes: Vec<DriveChange>, ids: &[&DocumentId]) -> Vec<DriveChange> {
    changes
        .into_iter()
        .filter(|c| ids.contains(&&c.document_id))
        .collect()
}

#[tokio::test]
async fn a_transaction_that_began_earlier_but_lands_later_is_delivered() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("feed-late");
    let (early, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create early");
    let start = manifest_cursor(&h, &owner).await;

    let held = test_db_pool().await.write_pool();
    let mut slow = held.begin().await.expect("begin slow transaction");
    let renamed = format!("{}.md", unique("late"));
    sqlx::query(
        "INSERT INTO document_path_changes (document_id, from_path, to_path) \
         VALUES ($1::text::uuid, $2, $3)",
    )
    .bind(early.id.as_str())
    .bind(&early.name)
    .bind(&renamed)
    .execute(&mut *slow)
    .await
    .expect("slow landing");

    let (fast, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create fast");
    let (withheld, resume) = drain(&h, &owner, &start).await;
    let served: Vec<_> = only(withheld, &[&early.id, &fast.id])
        .into_iter()
        .filter(|c| c.document_id == fast.id || c.kind != DriveChangeKind::Created)
        .collect();
    assert!(
        served.is_empty(),
        "nothing past an open older transaction is served: {served:?}"
    );

    slow.commit().await.expect("commit slow transaction");
    let (changes, _) = drain_settled(&h, &owner, &resume, |c| {
        c.iter().any(|c| c.document_id == fast.id)
    })
    .await;
    let kinds: Vec<_> = only(changes, &[&early.id, &fast.id])
        .into_iter()
        .filter(|c| c.document_id == fast.id || c.kind != DriveChangeKind::Created)
        .map(|c| (c.document_id, c.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                early.id.clone(),
                DriveChangeKind::Renamed {
                    from: early.name.clone()
                }
            ),
            (fast.id.clone(), DriveChangeKind::Created),
        ],
        "the late landing is delivered, ahead of the later transaction"
    );
}

#[tokio::test]
async fn an_open_transaction_that_lands_nothing_does_not_hold_the_feed_back() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("feed-idle");
    let start = manifest_cursor(&h, &owner).await;

    let held = test_db_pool().await.write_pool();
    let mut idle = held.begin().await.expect("begin unrelated transaction");
    sqlx::query("SELECT pg_current_xact_id()")
        .execute(&mut *idle)
        .await
        .expect("assign an id to the unrelated transaction");

    let (created, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let (changes, _) = drain_settled(&h, &owner, &start, |c| {
        c.iter().any(|c| c.document_id == created.id)
    })
    .await;
    let kinds: Vec<_> = only(changes, &[&created.id])
        .into_iter()
        .map(|c| c.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![DriveChangeKind::Created],
        "a landing after an unrelated open transaction is served"
    );
    idle.rollback()
        .await
        .expect("rollback unrelated transaction");
}

#[tokio::test]
async fn an_accepted_proposal_appears_in_the_feed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("feed-accept-owner");
    let proposer = person("feed-accept-proposer");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &proposer, AccessLevel::Comment).await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "ALPHA")],
    )
    .await
    .expect("propose");
    let start = manifest_cursor(&h, &owner).await;

    let review = ReviewRequest {
        proposal_id: proposed.proposal.id.clone(),
        note: None,
        reviewer: api_author(),
    };
    h.service
        .accept_proposal(&owner, review, &trace())
        .await
        .expect("accept");

    let (changes, _) = drain_settled(&h, &owner, &start, |c| {
        c.iter()
            .any(|c| c.revision_id.as_ref() == Some(&proposed.revision.id))
    })
    .await;
    let accepted: Vec<_> = only(changes, &[&document])
        .into_iter()
        .filter(|c| c.revision_id.as_ref() == Some(&proposed.revision.id))
        .collect();
    assert_eq!(accepted.len(), 1, "{accepted:?}");
    assert_eq!(accepted[0].kind, DriveChangeKind::Updated);
}

#[tokio::test]
async fn path_changes_and_revisions_interleave_in_landing_order() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("feed-interleave");
    let start = manifest_cursor(&h, &owner).await;
    let mut expected = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let (document, _) = h
            .service
            .create_from_template(&owner, create("blank-markdown", None), &trace())
            .await
            .expect("create");
        expected.push((document.id.clone(), DriveChangeKind::Created));
        let name = format!("{}.md", unique("interleaved"));
        h.service
            .update(
                &owner,
                &document.id,
                UpdateRequest {
                    name: Some(name),
                    folder: FolderMove::Keep,
                },
                &trace(),
            )
            .await
            .expect("rename");
        expected.push((
            document.id.clone(),
            DriveChangeKind::Renamed {
                from: document.name.clone(),
            },
        ));
        ids.push(document.id);
    }

    let refs: Vec<&DocumentId> = ids.iter().collect();
    let (changes, _) = drain_settled(&h, &owner, &start, |c| {
        only(c.to_vec(), &refs).len() >= expected.len()
    })
    .await;
    let changes = only(changes, &refs);
    let cursors: Vec<_> = changes
        .iter()
        .map(|c| parse_drive_cursor(&c.cursor).expect("cursor"))
        .collect();
    assert!(cursors.windows(2).all(|w| w[0] < w[1]));
    let unique_cursors: HashSet<_> = changes.iter().map(|c| c.cursor.clone()).collect();
    assert_eq!(unique_cursors.len(), changes.len(), "each change once");
    let kinds: Vec<_> = changes
        .into_iter()
        .map(|c| (c.document_id, c.kind))
        .collect();
    assert_eq!(kinds, expected);
}

#[test]
fn a_cursor_of_the_old_timestamp_shape_asks_for_a_reset() {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let old =
        URL_SAFE_NO_PAD.encode("2026-10-01T00:00:00+00:00|00000000-0000-0000-0000-000000000000");
    let refused = parse_drive_cursor(&old).expect_err("old cursor refused");
    assert!(matches!(refused, DocumentsError::CursorReset { .. }));
    assert!(refused.to_string().contains("restart from the manifest"));
}
