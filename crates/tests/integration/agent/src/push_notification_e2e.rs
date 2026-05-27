use anyhow::Result;
use systemprompt_agent::models::a2a::protocol::PushNotificationConfig;
use systemprompt_agent::repository::content::PushNotificationConfigRepository;
use systemprompt_identifiers::{ConfigId, TaskId};
use systemprompt_models::a2a::TaskState;

use crate::common::Fixture;

fn sample_config(url: &str) -> PushNotificationConfig {
    PushNotificationConfig {
        endpoint: url.to_string(),
        headers: Some(
            [(
                "x-custom".to_string(),
                serde_json::json!("custom-value"),
            )]
            .into_iter()
            .collect(),
        ),
        url: url.to_string(),
        token: Some("secret-token".to_string()),
        authentication: None,
    }
}

#[tokio::test]
async fn push_notification_add_and_get_roundtrip() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;

    let cfg = sample_config("https://example.test/hook");
    let config_id = repo.add_config(&task_id, &cfg).await?;
    assert!(!config_id.is_empty());

    let cid = ConfigId::new(&config_id);
    let fetched = repo.get_config(&task_id, &cid).await?;
    let fetched = fetched.expect("config should exist");
    assert_eq!(fetched.url, cfg.url);
    assert_eq!(fetched.token, cfg.token);
    assert!(fetched.headers.is_some());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_list_returns_all_configs() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;

    let _id1 = repo.add_config(&task_id, &sample_config("https://a.test")).await?;
    let _id2 = repo.add_config(&task_id, &sample_config("https://b.test")).await?;
    let _id3 = repo.add_config(&task_id, &sample_config("https://c.test")).await?;

    let list = repo.list_configs(&task_id).await?;
    assert_eq!(list.len(), 3);
    let urls: Vec<&str> = list.iter().map(|c| c.url.as_str()).collect();
    assert!(urls.contains(&"https://a.test"));
    assert!(urls.contains(&"https://b.test"));
    assert!(urls.contains(&"https://c.test"));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_list_empty_for_unknown_task() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;
    let unknown = TaskId::new("totally_unknown_task_xyz");
    let list = repo.list_configs(&unknown).await?;
    assert!(list.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_get_unknown_returns_none() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;
    let cid = ConfigId::new("nonexistent-config-id");
    let result = repo.get_config(&task_id, &cid).await?;
    assert!(result.is_none());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_delete_single() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;

    let cfg_id = repo.add_config(&task_id, &sample_config("https://del.test")).await?;
    let cid = ConfigId::new(&cfg_id);
    let deleted = repo.delete_config(&task_id, &cid).await?;
    assert!(deleted);

    let after = repo.get_config(&task_id, &cid).await?;
    assert!(after.is_none());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_delete_nonexistent_returns_false() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;
    let cid = ConfigId::new("not-real-id");
    let deleted = repo.delete_config(&task_id, &cid).await?;
    assert!(!deleted);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_delete_all_for_task_removes_all() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;

    let _ = repo.add_config(&task_id, &sample_config("https://1.test")).await?;
    let _ = repo.add_config(&task_id, &sample_config("https://2.test")).await?;
    let _ = repo.add_config(&task_id, &sample_config("https://3.test")).await?;

    let count = repo.delete_all_for_task(&task_id).await?;
    assert_eq!(count, 3);

    let after = repo.list_configs(&task_id).await?;
    assert!(after.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_delete_all_unknown_task_zero() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;
    let unknown = TaskId::new("absent_task_99999");
    let count = repo.delete_all_for_task(&unknown).await?;
    assert_eq!(count, 0);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_config_with_minimal_fields() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;

    let minimal = PushNotificationConfig {
        endpoint: "https://minimal.test/endpoint".to_string(),
        headers: None,
        url: "https://minimal.test".to_string(),
        token: None,
        authentication: None,
    };
    let cfg_id = repo.add_config(&task_id, &minimal).await?;
    let cid = ConfigId::new(&cfg_id);
    let fetched = repo.get_config(&task_id, &cid).await?.unwrap();
    assert_eq!(fetched.url, "https://minimal.test");
    assert!(fetched.token.is_none());
    assert!(fetched.headers.is_none());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_debug_impl_is_safe() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = PushNotificationConfigRepository::new(&fx.db)?;
    let dbg = format!("{:?}", repo);
    assert!(dbg.contains("PushNotificationConfigRepository"));
    fx.cleanup().await?;
    Ok(())
}
