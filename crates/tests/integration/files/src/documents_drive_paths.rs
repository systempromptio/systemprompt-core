use systemprompt_files::documents::service::{
    FolderMove, NewFolderRequest, UpdateRequest, parse_drive_cursor,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DriveChange, DriveChangeKind};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace, unique,
};

async fn drain(h: &Harness, who: &AuthenticatedUser, start: &str) -> Vec<DriveChange> {
    let mut cursor = parse_drive_cursor(start).expect("cursor parses");
    let mut seen: Vec<DriveChange> = Vec::new();
    for _ in 0..200 {
        let page = h
            .service
            .drive_changes(who, Some(cursor), 2, &trace())
            .await
            .expect("changes");
        let keys: Vec<_> = page
            .changes
            .iter()
            .map(|c| parse_drive_cursor(&c.cursor).expect("change cursor"))
            .map(|c| (c.updated_at, c.id))
            .collect();
        assert!(
            keys.windows(2).all(|w| w[0] < w[1]),
            "one page is one ordered stream across kinds"
        );
        for change in page.changes {
            if !seen.iter().any(|s| s.cursor == change.cursor) {
                seen.push(change);
            }
        }
        let next = parse_drive_cursor(&page.next_cursor).expect("next cursor");
        if next == cursor {
            break;
        }
        cursor = next;
    }
    seen.sort_by_key(|c| {
        let k = parse_drive_cursor(&c.cursor).expect("cursor");
        (k.updated_at, k.id)
    });
    seen
}

#[tokio::test]
async fn drive_feed_reports_rename_move_trash_and_restore_in_order() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("drive-owner");
    let stranger = person("drive-stranger");
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let folder_name = unique("moved");
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: folder_name.clone(),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id;
    let start = h
        .service
        .drive_manifest(&owner, &trace())
        .await
        .expect("manifest")
        .cursor;

    let renamed = format!("{}.md", unique("renamed"));
    let rename = UpdateRequest {
        name: Some(renamed.clone()),
        folder: FolderMove::Keep,
    };
    h.service
        .update(&owner, &document.id, rename, &trace())
        .await
        .expect("rename");
    let into = UpdateRequest {
        name: None,
        folder: FolderMove::Into(folder),
    };
    h.service
        .update(&owner, &document.id, into, &trace())
        .await
        .expect("move");
    h.service
        .trash(&owner, &document.id, &trace())
        .await
        .expect("trash");
    h.service
        .restore_document(&owner, &document.id, &trace())
        .await
        .expect("restore");

    let moved = format!("{folder_name}/{renamed}");
    let changes: Vec<_> = drain(&h, &owner, &start)
        .await
        .into_iter()
        .filter(|c| c.document_id == document.id)
        .filter(|c| !(c.kind == DriveChangeKind::Created && c.path == document.name))
        .collect();
    let kinds: Vec<_> = changes
        .iter()
        .map(|c| (c.kind.clone(), c.path.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                DriveChangeKind::Renamed {
                    from: document.name.clone()
                },
                renamed.clone()
            ),
            (
                DriveChangeKind::Renamed {
                    from: renamed.clone()
                },
                moved.clone()
            ),
            (DriveChangeKind::Deleted, moved.clone()),
            (DriveChangeKind::Created, moved.clone()),
        ]
    );
    assert!(changes[3].revision_id.is_some() && changes[3].content_sha256.is_some());
    assert!(changes[2].revision_id.is_none());

    let leaked = drain(&h, &stranger, &start).await;
    assert!(leaked.iter().all(|c| c.document_id != document.id));
}

#[tokio::test]
async fn drive_feed_reports_a_trashed_document_as_deleted_and_skips_its_revisions() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("drive-trash");
    let start = h
        .service
        .drive_manifest(&owner, &trace())
        .await
        .expect("manifest")
        .cursor;
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    h.service
        .trash(&owner, &document.id, &trace())
        .await
        .expect("trash");

    let changes: Vec<_> = drain(&h, &owner, &start)
        .await
        .into_iter()
        .filter(|c| c.document_id == document.id)
        .collect();
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].kind, DriveChangeKind::Deleted);
    assert_eq!(changes[0].path, document.name);
}
