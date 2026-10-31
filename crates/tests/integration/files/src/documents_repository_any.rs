use systemprompt_files::documents::models::{NewDocument, NewFolder};
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};
use systemprompt_models::documents::DocumentFormat;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn folder(repos: &DocumentRepositories, owner: &UserId) -> DocumentFolderId {
    repos
        .folders
        .insert(NewFolder {
            parent_id: None,
            name: unique("folder"),
            owner_user_id: owner.clone(),
            created_by: owner.clone(),
        })
        .await
        .expect("insert folder")
        .id
}

async fn document(repos: &DocumentRepositories, owner: &UserId) -> DocumentId {
    repos
        .documents
        .insert(NewDocument {
            folder_id: None,
            name: unique("doc"),
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

#[tokio::test]
async fn hard_delete_removes_the_row_once() {
    let repos = DocumentRepositories::new(&test_db_pool().await);
    let owner = UserId::new(unique("hard-delete"));
    let id = document(&repos, &owner).await;
    assert!(repos.documents.hard_delete(&id).await.expect("delete"));
    assert!(repos.documents.find(&id).await.expect("find").is_none());
    assert!(!repos.documents.hard_delete(&id).await.expect("again"));
}

#[tokio::test]
async fn move_to_any_files_into_a_live_folder_back_to_root_and_refuses_a_deleted_one() {
    let repos = DocumentRepositories::new(&test_db_pool().await);
    let owner = UserId::new(unique("move-any"));
    let id = document(&repos, &owner).await;
    let live = folder(&repos, &owner).await;

    assert!(
        repos
            .documents
            .move_to_any(&id, Some(&live))
            .await
            .expect("move")
    );
    let moved = repos.documents.find(&id).await.expect("find").expect("doc");
    assert_eq!(moved.folder_id, Some(live.clone()));

    assert!(repos.documents.move_to_any(&id, None).await.expect("root"));
    let rooted = repos.documents.find(&id).await.expect("find").expect("doc");
    assert_eq!(rooted.folder_id, None);

    let gone = folder(&repos, &owner).await;
    assert!(repos.folders.soft_delete(&gone).await.expect("soft delete"));
    assert!(
        !repos
            .documents
            .move_to_any(&id, Some(&gone))
            .await
            .expect("refused")
    );
    let kept = repos.documents.find(&id).await.expect("find").expect("doc");
    assert_eq!(kept.folder_id, None);

    let missing = DocumentId::new(Uuid::new_v4().to_string());
    assert!(
        !repos
            .documents
            .move_to_any(&missing, None)
            .await
            .expect("missing")
    );
}
