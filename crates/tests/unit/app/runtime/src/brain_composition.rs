use std::sync::Arc;

use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_runtime::{BrainCore, BrainInputs, brain_sink};
use systemprompt_test_fixtures::closed_db_pool;

async fn core_for(enabled: bool) -> Option<BrainCore> {
    let database = closed_db_pool().await;
    let listener_pool = database.write_pool().as_ref().clone();
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled,
        ..BrainProfile::default()
    };
    BrainCore::new(&BrainInputs {
        database: &database,
        instance_id: &InstanceId::new("brain-composition"),
        listener_pool: &listener_pool,
        notifications: &notifications,
        profile: &profile,
    })
}

#[tokio::test]
async fn disabled_brain_composes_nothing_and_leaves_the_sink_unavailable() {
    let core = core_for(false).await;

    assert!(core.is_none());
    assert!(!brain_sink(core.as_ref()).is_available());
}

#[tokio::test]
async fn enabled_brain_composes_a_core_whose_sink_reaches_the_outbox() {
    let core = core_for(true).await;

    assert!(core.is_some());
    assert!(brain_sink(core.as_ref()).is_available());
}
