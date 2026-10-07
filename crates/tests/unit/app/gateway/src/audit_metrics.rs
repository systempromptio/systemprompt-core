//! `record_completion`: one upstream and one overhead sample per completed
//! request, the overhead being the request minus the upstream bracket, and no
//! sample at all when the upstream bracket never closed.

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use systemprompt_gateway::audit::metrics::{
    GATEWAY_OVERHEAD_SECONDS, GATEWAY_UPSTREAM_DURATION_SECONDS, OVERHEAD_BUCKETS,
    UPSTREAM_BUCKETS, record_completion,
};

fn samples(body: impl FnOnce()) -> Vec<(String, Vec<(String, String)>, Vec<f64>)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, body);
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(composite, _, _, value)| {
            let key = composite.key();
            let labels = key
                .labels()
                .map(|l| (l.key().to_owned(), l.value().to_owned()))
                .collect();
            match value {
                DebugValue::Histogram(values) => Some((
                    key.name().to_owned(),
                    labels,
                    values.into_iter().map(|v| v.into_inner()).collect(),
                )),
                _ => None,
            }
        })
        .collect()
}

fn histogram<'a>(
    all: &'a [(String, Vec<(String, String)>, Vec<f64>)],
    name: &str,
) -> Option<&'a (String, Vec<(String, String)>, Vec<f64>)> {
    all.iter().find(|(n, _, _)| n == name)
}

#[test]
fn a_completion_records_upstream_and_overhead_in_seconds() {
    let all = samples(|| record_completion("anthropic.messages", "anthropic", 1250, Some(1200)));

    let (_, labels, upstream) =
        histogram(&all, GATEWAY_UPSTREAM_DURATION_SECONDS).expect("upstream histogram");
    assert_eq!(upstream, &vec![1.2]);
    assert!(labels.contains(&("route".to_owned(), "anthropic.messages".to_owned())));
    assert!(labels.contains(&("provider".to_owned(), "anthropic".to_owned())));

    let (_, _, overhead) = histogram(&all, GATEWAY_OVERHEAD_SECONDS).expect("overhead histogram");
    assert_eq!(overhead, &vec![0.05]);
}

#[test]
fn no_upstream_bracket_records_nothing() {
    let all = samples(|| record_completion("openai.chat", "openai", 900, None));
    assert!(all.is_empty(), "{all:?}");
}

#[test]
fn clock_skew_never_records_negative_overhead() {
    let all = samples(|| record_completion("openai.chat", "openai", 100, Some(150)));
    let (_, _, overhead) = histogram(&all, GATEWAY_OVERHEAD_SECONDS).expect("overhead histogram");
    assert_eq!(overhead, &vec![0.0]);
}

#[test]
fn bucket_bounds_are_ascending_and_sized_for_their_signal() {
    assert!(OVERHEAD_BUCKETS.windows(2).all(|w| w[0] < w[1]));
    assert!(UPSTREAM_BUCKETS.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(OVERHEAD_BUCKETS.first(), Some(&0.001));
    assert_eq!(UPSTREAM_BUCKETS.last(), Some(&120.0));
}
