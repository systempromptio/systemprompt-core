use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::NewFolderRequest;
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, deny, harness, harness_on, person, trace, unique,
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

async fn document_in(h: &Harness, who: &AuthenticatedUser, at: &DocumentFolderId) -> DocumentId {
    let mut request = create("blank-text", Some(at));
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

#[tokio::test]
async fn rename_folder_updates_the_name_and_refuses_bad_names() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-rename");
    let id = folder(&h, &owner, None).await;
    let name = unique("Renamed");
    let renamed = h
        .service
        .rename_folder(&owner, &id, &name, &trace())
        .await
        .expect("rename");
    assert_eq!(renamed.name, name);
    assert_eq!(renamed.id, id);
    let bad = h
        .service
        .rename_folder(&owner, &id, "a/b", &trace())
        .await
        .expect_err("invalid");
    assert!(matches!(bad, DocumentsError::InvalidName { .. }));
}

#[tokio::test]
async fn move_folder_reparents_and_refuses_cycles() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-move");
    let parent = folder(&h, &owner, None).await;
    let child = folder(&h, &owner, None).await;
    let moved = h
        .service
        .move_folder(&owner, &child, Some(&parent), &trace())
        .await
        .expect("move");
    assert_eq!(moved.parent_id.as_ref(), Some(&parent));
    let into_self = h
        .service
        .move_folder(&owner, &child, Some(&child), &trace())
        .await
        .expect_err("self");
    assert!(matches!(into_self, DocumentsError::InvalidMove { .. }));
    let into_descendant = h
        .service
        .move_folder(&owner, &parent, Some(&child), &trace())
        .await
        .expect_err("cycle");
    assert!(matches!(
        into_descendant,
        DocumentsError::InvalidMove { .. }
    ));
    let to_root = h
        .service
        .move_folder(&owner, &child, None, &trace())
        .await
        .expect("root");
    assert!(to_root.parent_id.is_none());
}

#[tokio::test]
async fn delete_folder_refuses_non_empty_unless_recursive() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-delete");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let doc = document_in(&h, &owner, &sub).await;
    let refused = h
        .service
        .delete_folder(&owner, &root, false, &trace())
        .await
        .expect_err("not empty");
    assert!(matches!(refused, DocumentsError::FolderNotEmpty { .. }));
    let with_doc = h
        .service
        .delete_folder(&owner, &sub, false, &trace())
        .await
        .expect_err("holds a document");
    assert!(matches!(with_doc, DocumentsError::FolderNotEmpty { .. }));

    h.service
        .delete_folder(&owner, &root, true, &trace())
        .await
        .expect("recursive");
    for id in [&root, &sub] {
        assert!(h.repos.folders.find(id).await.expect("find").is_none());
    }
    assert!(h.repos.documents.find(&doc).await.expect("find").is_none());
    let gone = h
        .service
        .rename_folder(&owner, &root, "After", &trace())
        .await
        .expect_err("deleted");
    assert!(!matches!(gone, DocumentsError::InvalidName { .. }));
}

#[tokio::test]
async fn empty_folder_deletes_without_recursion() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-empty");
    let id = folder(&h, &owner, None).await;
    h.service
        .delete_folder(&owner, &id, false, &trace())
        .await
        .expect("delete");
    assert!(h.repos.folders.find(&id).await.expect("find").is_none());
}

#[tokio::test]
async fn deny_hook_refuses_folder_mutations() {
    let open = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-deny");
    let id = folder(&open, &owner, None).await;
    let closed = harness_on(deny(), GroupSource::new(&[]), open.storage.clone()).await;
    let stranger = person("folder-stranger");
    let rename = closed
        .service
        .rename_folder(&stranger, &id, "Nope", &trace())
        .await
        .expect_err("rename");
    assert!(
        matches!(
            rename,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{rename:?}"
    );
    let moved = closed
        .service
        .move_folder(&stranger, &id, None, &trace())
        .await
        .expect_err("move");
    assert!(
        matches!(
            moved,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{moved:?}"
    );
    let deleted = closed
        .service
        .delete_folder(&stranger, &id, true, &trace())
        .await
        .expect_err("delete");
    assert!(
        matches!(
            deleted,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{deleted:?}"
    );
    let row = open
        .repos
        .folders
        .find(&id)
        .await
        .expect("find")
        .expect("row");
    assert!(row.deleted_at.is_none());
    assert_ne!(row.name, "Nope");
}
