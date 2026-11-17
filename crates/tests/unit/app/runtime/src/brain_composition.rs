use std::sync::Arc;

use systemprompt_extension::DynNotificationSink;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::{closed_db_pool, fixture_brain};

async fn handle_for(enabled: bool) -> Option<Arc<systemprompt_brain::BrainHandle>> {
    let database = closed_db_pool().await;
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled,
        ..BrainProfile::default()
    };
    fixture_brain(&database, None, notifications, profile)
}

#[tokio::test]
async fn disabled_brain_composes_nothing() {
    assert!(handle_for(false).await.is_none());
}

#[tokio::test]
async fn enabled_brain_composes_a_handle_whose_sink_reaches_the_outbox() {
    let handle = handle_for(true)
        .await
        .expect("an enabled brain composes a handle");

    assert!(handle.sink().is_available());
}
