use std::sync::Arc;

use systemprompt_extension::DynNotificationSink;
use systemprompt_extension::brain::{BrainModelClient, BrainModelError};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse};
use systemprompt_runtime::brain::BrainHandle;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_runtime::{BrainCore, BrainInputs, RecallSeams};

use crate::documents_mcp_rig::{Rig, rig};

struct UnavailableModel;

#[async_trait::async_trait]
impl BrainModelClient for UnavailableModel {
    async fn complete(
        &self,
        _request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        Err(BrainModelError::Unavailable)
    }
}

pub(crate) fn handle_for(r: &Rig) -> Arc<BrainHandle> {
    let listener_pool = r.pool.write_pool().as_ref().clone();
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled: true,
        ..BrainProfile::default()
    };
    let core = BrainCore::new(&BrainInputs {
        database: &r.pool,
        instance_id: &systemprompt_test_fixtures::unique_instance(),
        listener_pool: &listener_pool,
        notifications: &notifications,
        profile: &profile,
    })
    .expect("enabled core");
    core.assemble(&r.runtime, RecallSeams::default())
}

#[tokio::test]
async fn starting_the_consumer_twice_is_refused_and_keeps_the_first() {
    let r = rig(false).await;
    let handle = handle_for(&r);
    let app = systemprompt_test_fixtures::test_app_context(
        &r.pool,
        &systemprompt_test_fixtures::test_database_url(),
    );
    assert!(handle.consumer().is_none());

    let first = handle
        .start_consumer(app.as_ref(), Arc::new(UnavailableModel))
        .expect("the first start binds the consumer slot");
    let second = handle.start_consumer(app.as_ref(), Arc::new(UnavailableModel));

    assert!(second.is_err(), "a second start must not rebind the slot");
    let held = handle.consumer().expect("consumer recorded");
    assert!(Arc::ptr_eq(held, &first));
    assert!(format!("{handle:?}").contains("consumer: true"));
}
