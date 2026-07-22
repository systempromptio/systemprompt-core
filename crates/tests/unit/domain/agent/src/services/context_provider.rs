// Drives ContextProviderService through the full ContextProvider trait
// lifecycle against a real database: create, list-with-stats, get, rename,
// and delete, plus the NotFound mapping for lookups of unknown contexts.

use systemprompt_agent::services::ContextProviderService;
use systemprompt_identifiers::ContextId;
use systemprompt_traits::{ContextProvider, ContextProviderError};

use crate::repository::{seed_user_and_session, try_pool};

#[tokio::test]
async fn context_lifecycle_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user, session) = seed_user_and_session(&pool).await;
    let provider = ContextProviderService::new(&pool).expect("provider");

    let ctx_id = provider
        .create_context(&user, Some(&session), "provider ctx")
        .await
        .expect("create context");

    let listed = provider
        .list_contexts_with_stats(&user)
        .await
        .expect("list contexts");
    let entry = listed
        .iter()
        .find(|c| c.context_id == ctx_id)
        .expect("created context listed");
    assert_eq!(entry.name.as_str(), ("provider ctx"));
    assert_eq!(entry.task_count, 0);
    assert_eq!(entry.message_count, 0);

    let fetched = provider
        .get_context(&ctx_id, &user)
        .await
        .expect("get context");
    assert_eq!(fetched.user_id, user);

    provider
        .update_context_name(&ctx_id, &user, "renamed ctx")
        .await
        .expect("rename");
    let renamed = provider.get_context(&ctx_id, &user).await.expect("get");
    assert_eq!(renamed.name.as_str(), ("renamed ctx"));

    provider
        .delete_context(&ctx_id, &user)
        .await
        .expect("delete");
    let missing = provider.get_context(&ctx_id, &user).await;
    assert!(
        matches!(missing, Err(ContextProviderError::NotFound(_))),
        "deleted context must map to NotFound, got {missing:?}"
    );
}

#[tokio::test]
async fn unknown_context_lookups_map_to_not_found() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user, _session) = seed_user_and_session(&pool).await;
    let provider = ContextProviderService::new(&pool).expect("provider");
    let ghost = ContextId::generate();

    let get = provider.get_context(&ghost, &user).await;
    assert!(matches!(get, Err(ContextProviderError::NotFound(_))));

    let rename = provider.update_context_name(&ghost, &user, "x").await;
    assert!(matches!(rename, Err(ContextProviderError::NotFound(_))));

    let delete = provider.delete_context(&ghost, &user).await;
    assert!(matches!(delete, Err(ContextProviderError::NotFound(_))));
}
