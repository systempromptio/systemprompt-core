use std::sync::Arc;

use systemprompt_extension::DynNotificationSink;
use systemprompt_extension::brain::{BrainModelClient, BrainModelError};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse};
use systemprompt_brain::BrainHandle;
use systemprompt_runtime::brain::mcp::BrainMcpTools;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::fixture_brain;

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
    let handle = handle_for(&r);
    assert!(handle.consumer().is_none());

    let first = handle
        .start_consumer(Arc::new(UnavailableModel))
        .expect("the first start binds the consumer slot");
    let second = handle.start_consumer(Arc::new(UnavailableModel));

    assert!(second.is_err(), "a second start must not rebind the slot");
    let held = handle.consumer().expect("consumer recorded");
    assert!(Arc::ptr_eq(held, &first));
    assert!(format!("{handle:?}").contains("consumer: true"));
}
