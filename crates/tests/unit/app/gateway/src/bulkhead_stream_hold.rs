use std::time::Duration;

use futures_util::StreamExt;
use systemprompt_gateway::protocol::outbound::OutboundOutcome;
use systemprompt_gateway::service::failover::{DeploymentLoad, DeploymentSlot};
use systemprompt_storage::resilience::Bulkhead;

fn open_stream() -> OutboundOutcome {
    OutboundOutcome::RawStreaming {
        content_type: None,
        stream: futures_util::stream::pending().boxed(),
    }
}

#[tokio::test]
async fn a_streamed_body_holds_its_permit_until_it_is_dropped() {
    let streams = 3;
    let bulkhead = Bulkhead::new("deployment", streams - 1);
    let load = DeploymentLoad::default();
    let mut bodies = Vec::new();
    for _ in 0..streams - 1 {
        let permit = bulkhead.try_acquire().expect("a permit under the cap");
        let slot = DeploymentSlot::new(Some(permit), load.enter("deployment"));
        bodies.push(slot.hold(open_stream()));
    }
    assert!(
        bulkhead.try_acquire().is_err(),
        "the stream past the cap is refused while every body is open"
    );
    assert_eq!(load.in_flight("deployment"), (streams - 1) as u64);

    drop(bodies.pop());
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        bulkhead.try_acquire().is_ok(),
        "dropping a body releases its permit"
    );
    assert_eq!(load.in_flight("deployment"), (streams - 2) as u64);
}
