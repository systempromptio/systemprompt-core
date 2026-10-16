//! Gateway phase histogram and the outbound W3C trace context.

use std::time::Duration;

use metrics_util::debugging::DebuggingRecorder;
use systemprompt_gateway::protocol::outbound::trace_context::{current_traceparent, with_trace};
use systemprompt_gateway::{GATEWAY_PHASE_SECONDS, GatewayPhase};
use systemprompt_identifiers::TraceId;

#[test]
fn every_phase_records_under_its_own_label() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        for phase in GatewayPhase::ALL {
            systemprompt_gateway::audit::phases::record(phase, Duration::from_millis(5));
        }
    });
    let mut labels: Vec<String> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(composite, _, _, _)| composite.key().name() == GATEWAY_PHASE_SECONDS)
        .flat_map(|(composite, _, _, _)| {
            composite
                .key()
                .labels()
                .map(|l| l.value().to_owned())
                .collect::<Vec<_>>()
        })
        .collect();
    labels.sort();
    let mut expected: Vec<String> = GatewayPhase::ALL
        .iter()
        .map(|p| p.label().to_owned())
        .collect();
    expected.sort();
    assert_eq!(labels, expected);
}

#[tokio::test]
async fn the_dispatch_trace_id_is_stamped_as_a_traceparent() {
    let trace = TraceId::new("4bf92f3577b34da6a3ce929d0e0e4736");
    let header = with_trace(Some(trace), async { current_traceparent() })
        .await
        .expect("a W3C trace id is propagated");
    assert!(
        header.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"),
        "{header}"
    );
}

#[tokio::test]
async fn no_traceparent_outside_a_dispatch_or_for_a_non_w3c_id() {
    assert!(current_traceparent().is_none());
    let none = with_trace(Some(TraceId::system()), async { current_traceparent() }).await;
    assert!(none.is_none());
}
