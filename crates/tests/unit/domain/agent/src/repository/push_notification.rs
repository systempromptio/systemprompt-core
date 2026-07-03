use super::{repos, seed_context_and_task, seed_user_and_session, try_pool};
use systemprompt_agent::models::a2a::protocol::PushNotificationConfig;
use systemprompt_identifiers::{ConfigId, TaskId};

fn make_config(url: &str) -> PushNotificationConfig {
    let mut headers = serde_json::Map::new();
    headers.insert(
        "X-Token".to_owned(),
        serde_json::Value::String("abc".to_owned()),
    );
    PushNotificationConfig {
        endpoint: format!("/webhook/{url}"),
        headers: Some(headers),
        url: url.to_owned(),
        token: Some("secret-token".to_owned()),
        authentication: None,
    }
}

#[tokio::test]
async fn add_get_and_list_config() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let config = make_config("https://example.invalid/hook");
    let config_id_str = r
        .push_notification_configs
        .add_config(&task_id, &config)
        .await
        .expect("add");
    let config_id = ConfigId::new(config_id_str);

    let fetched = r
        .push_notification_configs
        .get_config(&task_id, &config_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.url, "https://example.invalid/hook");
    assert_eq!(fetched.endpoint, "/webhook/https://example.invalid/hook");
    assert_eq!(fetched.token.as_deref(), Some("secret-token"));
    let headers = fetched.headers.expect("headers persisted");
    assert_eq!(headers.get("X-Token").and_then(|v| v.as_str()), Some("abc"));

    let list = r
        .push_notification_configs
        .list_configs(&task_id)
        .await
        .expect("list");
    assert_eq!(list.len(), 1);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_config_unknown_returns_none() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let result = r
        .push_notification_configs
        .get_config(&task_id, &ConfigId::generate())
        .await
        .expect("get");
    assert!(result.is_none());

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn delete_config() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let config_id = ConfigId::new(
        r.push_notification_configs
            .add_config(&task_id, &make_config("https://h.invalid"))
            .await
            .expect("add"),
    );

    let deleted = r
        .push_notification_configs
        .delete_config(&task_id, &config_id)
        .await
        .expect("delete");
    assert!(deleted);

    let deleted_again = r
        .push_notification_configs
        .delete_config(&task_id, &config_id)
        .await
        .expect("delete again");
    assert!(!deleted_again);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn delete_all_for_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    r.push_notification_configs
        .add_config(&task_id, &make_config("https://a.invalid"))
        .await
        .expect("add 1");
    r.push_notification_configs
        .add_config(&task_id, &make_config("https://b.invalid"))
        .await
        .expect("add 2");

    let removed = r
        .push_notification_configs
        .delete_all_for_task(&task_id)
        .await
        .expect("delete all");
    assert_eq!(removed, 2);

    let list = r
        .push_notification_configs
        .list_configs(&task_id)
        .await
        .expect("list");
    assert!(list.is_empty());

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn list_configs_empty_for_unknown_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let list = r
        .push_notification_configs
        .list_configs(&TaskId::generate())
        .await
        .expect("list");
    assert!(list.is_empty());
}
