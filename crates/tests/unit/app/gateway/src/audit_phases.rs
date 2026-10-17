//! Gateway phase histogram, the in-flight stream gauge, and the outbound
//! W3C trace context.

use std::time::Duration;

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use systemprompt_gateway::protocol::outbound::trace_context::{current_traceparent, with_trace};
use systemprompt_gateway::{
    GATEWAY_PHASE_SECONDS, GATEWAY_STREAMS_IN_FLIGHT, GatewayPhase, StreamInFlight,
};
use systemprompt_identifiers::TraceId;

fn gauge(snapshotter: &Snapshotter, name: &str) -> Option<f64> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find_map(|(composite, _, _, value)| match value {
            DebugValue::Gauge(v) if composite.key().name() == name => Some(v.into_inner()),
            _ => None,
        })
}

fn phase_samples(snapshotter: &Snapshotter, phase: &str) -> Vec<f64> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(composite, _, _, value)| {
            let key = composite.key();
            let matches = key.name() == GATEWAY_PHASE_SECONDS
                && key
                    .labels()
                    .any(|l| l.key() == "phase" && l.value() == phase);
            match value {
                DebugValue::Histogram(values) if matches => Some(
                    values
                        .into_iter()
                        .map(|v| v.into_inner())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            }
        })
        .flatten()
        .collect()
}

#[test]
fn a_stream_holds_the_gauge_until_its_body_is_dropped() {
    let open = DebuggingRecorder::new();
    let open_snap = open.snapshotter();
    metrics::with_local_recorder(&open, || {
        let first = StreamInFlight::enter();
        let second = StreamInFlight::enter();
        std::mem::forget((first, second));
    });
    assert_eq!(gauge(&open_snap, GATEWAY_STREAMS_IN_FLIGHT), Some(2.0));

    let closed = DebuggingRecorder::new();
    let closed_snap = closed.snapshotter();
    metrics::with_local_recorder(&closed, || {
        let first = StreamInFlight::enter();
        let second = StreamInFlight::enter();
        drop(first);
        drop(second);
    });
    assert_eq!(gauge(&closed_snap, GATEWAY_STREAMS_IN_FLIGHT), Some(0.0));
}

#[test]
fn a_dropped_stream_records_its_lifetime_as_the_stream_phase() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        let guard = StreamInFlight::enter();
        std::thread::sleep(Duration::from_millis(30));
        drop(guard);
    });
    let samples = phase_samples(&snapshotter, "stream");
    assert_eq!(samples.len(), 1);
    assert!(samples[0] >= 0.03, "{samples:?}");
}

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
    let header = with_trace(Some(trace), None, async { current_traceparent() })
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
    let none = with_trace(Some(TraceId::system()), None, async {
        current_traceparent()
    })
    .await;
    assert!(none.is_none());
}

#[tokio::test]
async fn an_originated_trace_is_marked_sampled() {
    let trace = TraceId::new("4bf92f3577b34da6a3ce929d0e0e4736");
    let header = with_trace(Some(trace), None, async { current_traceparent() })
        .await
        .expect("propagated");
    assert!(header.ends_with("-01"), "{header}");
}

#[tokio::test]
async fn the_callers_unsampled_flag_is_propagated() {
    let inbound = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
    let trace = TraceId::from_traceparent(inbound).expect("valid traceparent");
    let sampled = trace.sampled_in_traceparent(inbound);
    assert_eq!(sampled, Some(false));
    let header = with_trace(Some(trace), sampled, async { current_traceparent() })
        .await
        .expect("propagated");
    assert!(header.ends_with("-00"), "{header}");
}

#[test]
fn a_traceparent_for_another_trace_carries_no_flag() {
    let trace = TraceId::new("0af7651916cd43dd8448eb211c80319c");
    let other = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
    assert_eq!(trace.sampled_in_traceparent(other), None);
    assert_eq!(trace.sampled_in_traceparent("garbage"), None);
}
