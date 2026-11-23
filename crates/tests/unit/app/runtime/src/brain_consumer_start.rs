use std::sync::Arc;

use systemprompt_brain::BrainHandle;
use systemprompt_brain::mcp::BrainMcpTools;
use systemprompt_extension::DynNotificationSink;
use systemprompt_extension::brain::{BrainModelClient, BrainModelError, DynBrainConsumerStart};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse};
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::{fixture_brain, fixture_brain_kernel, fixture_brain_slot};

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
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled: true,
        ..BrainProfile::default()
    };
    fixture_brain(&r.pool, Some(&r.runtime), notifications, profile).expect("enabled brain")
}

pub(crate) fn tools(handle: &BrainHandle) -> BrainMcpTools {
    let (service, recall, profile) = handle.routes();
    BrainMcpTools::from_parts(service, recall, profile).expect("tools")
}

#[tokio::test]
async fn starting_the_consumer_twice_is_refused_and_keeps_the_first() {
    let r = rig(false).await;
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let kernel = fixture_brain_kernel(
        &r.pool,
        Some(&r.runtime),
        notifications,
        BrainProfile {
            enabled: true,
            ..BrainProfile::default()
        },
    );
    let handle = Arc::clone(fixture_brain_slot(&kernel).get().expect("enabled brain"));
    assert!(handle.consumer().is_none());
    let start = kernel
        .state()
        .get::<DynBrainConsumerStart>()
        .expect("the brain member provides the consumer start")
        .clone();

    let first = start
        .start(Arc::new(UnavailableModel))
        .expect("the first start binds the consumer slot");
    assert!(first.is_some(), "an enabled brain starts its consumer");
    let second = start.start(Arc::new(UnavailableModel));

    assert!(second.is_err(), "a second start must not rebind the slot");
    assert!(handle.consumer().is_some(), "consumer recorded");
    assert!(format!("{handle:?}").contains("consumer: true"));
}
