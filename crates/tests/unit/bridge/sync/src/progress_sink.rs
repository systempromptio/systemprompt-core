//! The sink the GUI installs for the duration of a sync, and the label each
//! step renders to.

use std::sync::{Arc, Mutex};

use systemprompt_bridge::progress::{SyncProgress, SyncProgressSink};

fn recorder() -> (SyncProgressSink, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = SyncProgressSink::default();
    let captured = Arc::clone(&seen);
    sink.install(Arc::new(move |p: &SyncProgress| {
        captured
            .lock()
            .expect("recorder lock")
            .push(format!("{}:{}", p.phase, p.label()));
    }));
    (sink, seen)
}

#[test]
fn a_single_item_step_labels_itself_without_a_counter() {
    let step = SyncProgress::new("fetching", "manifest", 1, 1);
    assert_eq!(step.label(), "manifest");
    assert_eq!(step.phase, "fetching");
}

#[test]
fn a_step_out_of_many_labels_itself_with_its_position() {
    let step = SyncProgress::new("downloading", "commons", 3, 12);
    assert_eq!(step.label(), "commons (3/12)");
}

#[test]
fn a_zero_total_still_labels_without_a_counter_rather_than_printing_out_of_zero() {
    let step = SyncProgress::new("staging", "nothing", 0, 0);
    assert_eq!(step.label(), "nothing");
}

#[test]
fn reporting_into_a_sink_nobody_installed_is_a_silent_no_op() {
    let sink = SyncProgressSink::default();
    sink.report(&SyncProgress::new("fetching", "manifest", 1, 1));
    assert_eq!(
        format!("{sink:?}"),
        "SyncProgressSink { .. }",
        "the sink must not print the closure it holds"
    );
}

#[test]
fn an_installed_sink_sees_every_step_in_the_order_it_was_reported() {
    let (sink, seen) = recorder();

    sink.report(&SyncProgress::new("fetching", "manifest", 1, 1));
    sink.report(&SyncProgress::new("downloading", "commons", 1, 2));
    sink.report(&SyncProgress::new("downloading", "development", 2, 2));

    assert_eq!(
        *seen.lock().expect("lock"),
        vec![
            "fetching:manifest".to_owned(),
            "downloading:commons (1/2)".to_owned(),
            "downloading:development (2/2)".to_owned(),
        ]
    );
}

#[test]
fn clearing_the_sink_stops_delivery_without_losing_what_was_already_reported() {
    let (sink, seen) = recorder();
    sink.report(&SyncProgress::new("fetching", "manifest", 1, 1));

    sink.clear();
    sink.report(&SyncProgress::new("downloading", "commons", 1, 2));

    assert_eq!(
        *seen.lock().expect("lock"),
        vec!["fetching:manifest".to_owned()]
    );
}

#[test]
fn installing_a_second_sink_replaces_the_first_rather_than_fanning_out() {
    let (sink, first) = recorder();
    let second = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&second);
    sink.install(Arc::new(move |p: &SyncProgress| {
        captured.lock().expect("lock").push(p.label());
    }));

    sink.report(&SyncProgress::new("staging", "swap", 1, 1));

    assert!(first.lock().expect("lock").is_empty());
    assert_eq!(*second.lock().expect("lock"), vec!["swap".to_owned()]);
}

#[test]
fn a_clone_of_the_sink_shares_the_installed_reporter() {
    let (sink, seen) = recorder();
    let clone = sink.clone();

    clone.report(&SyncProgress::new("staging", "swap", 1, 1));
    assert_eq!(*seen.lock().expect("lock"), vec!["staging:swap".to_owned()]);

    clone.clear();
    sink.report(&SyncProgress::new("staging", "again", 1, 1));
    assert_eq!(seen.lock().expect("lock").len(), 1);
}
