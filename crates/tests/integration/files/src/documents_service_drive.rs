use std::collections::HashSet;

use systemprompt_files::documents::service::parse_drive_cursor;
use systemprompt_identifiers::{DocumentId, DriveDeviceId};
use systemprompt_models::documents::{
    DriveChangeKind, DriveCommitMeta, DriveCommitResponse, content_hash,
};

use crate::documents_service_fixture::{
    GroupSource, allow, create, harness, person, trace, unique,
};

#[tokio::test]
async fn drive_changes_hide_invisible_documents_and_page_by_cursor() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = person("reader");
    let stranger = person("stranger");
    let start = h
        .service
        .drive_manifest(&reader, &trace())
        .await
        .expect("manifest")
        .cursor;

    let mut mine = Vec::new();
    for _ in 0..3 {
        let (document, _) = h
            .service
            .create_from_template(&reader, create("blank-markdown", None), &trace())
            .await
            .expect("create visible");
        mine.push(document.id);
    }
    let (hidden_document, _) = h
        .service
        .create_from_template(&stranger, create("blank-markdown", None), &trace())
        .await
        .expect("create invisible");
    let hidden = hidden_document.id;

    let mut cursor = parse_drive_cursor(&start).expect("manifest cursor parses");
    let mut seen: HashSet<DocumentId> = HashSet::new();
    let mut pages = 0;
    for _ in 0..500 {
        let page = h
            .service
            .drive_changes(&reader, Some(cursor), 2, &trace())
            .await
            .expect("changes");
        pages += 1;
        for change in &page.changes {
            assert_ne!(change.document_id, hidden, "an invisible document leaked");
            assert!(matches!(
                change.kind,
                DriveChangeKind::Created | DriveChangeKind::Updated
            ));
            seen.insert(change.document_id.clone());
        }
        let next = parse_drive_cursor(&page.next_cursor).expect("next cursor parses");
        if mine.iter().all(|id| seen.contains(id)) || next == cursor {
            break;
        }
        assert!(next > cursor, "the feed must advance");
        cursor = next;
    }
    assert!(
        pages > 1,
        "three documents at two per page take several pages"
    );
    for id in &mine {
        assert!(
            seen.contains(id),
            "visible document {id} missing from the feed"
        );
    }

    let manifest = h
        .service
        .drive_manifest(&reader, &trace())
        .await
        .expect("manifest");
    assert!(manifest.entries.iter().all(|e| e.document_id != hidden));
    assert!(
        mine.iter()
            .all(|id| manifest.entries.iter().any(|e| &e.document_id == id))
    );
}

#[tokio::test]
async fn drive_commit_creates_by_path_then_conflicts_on_a_stale_base() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let user = person("bridge");
    let name = format!("{}.md", unique("synced"));
    let bytes = b"hello\n".to_vec();
    let meta = |document_id, base_revision, bytes: &[u8]| DriveCommitMeta {
        document_id,
        path: name.clone(),
        base_revision,
        content_sha256: content_hash(bytes),
        size_bytes: bytes.len() as u64,
        device_id: DriveDeviceId::new("laptop"),
        message: None,
    };
    let created = h
        .service
        .drive_commit(&user, meta(None, None, &bytes), bytes.clone(), &trace())
        .await
        .expect("create by path");
    let DriveCommitResponse::Committed { revision_id } = created else {
        panic!("expected Committed, got {created:?}");
    };
    let document = h
        .repos
        .documents
        .find_in_folder(None, &name)
        .await
        .expect("find")
        .expect("created at the root");

    let edited = b"hello again\n".to_vec();
    let next = h
        .service
        .drive_commit(
            &user,
            meta(
                Some(document.id.clone()),
                Some(revision_id.clone()),
                &edited,
            ),
            edited.clone(),
            &trace(),
        )
        .await
        .expect("commit on head");
    assert!(matches!(next, DriveCommitResponse::Committed { .. }));

    let blind = h
        .service
        .drive_commit(&user, meta(None, None, &edited), edited.clone(), &trace())
        .await
        .expect("path of an existing document without a base");
    assert!(
        matches!(blind, DriveCommitResponse::Conflict { .. }),
        "{blind:?}"
    );

    let corrupted = h
        .service
        .drive_commit(
            &user,
            meta(Some(document.id), None, b"other"),
            edited,
            &trace(),
        )
        .await
        .expect_err("hash mismatch");
    assert!(corrupted.to_string().contains("corrupted"));
}
