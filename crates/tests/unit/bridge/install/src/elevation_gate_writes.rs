//! Every elevated write runs through the injected gate: distinct writes never
//! overlap each other or a profile install, a write started by the thread that
//! holds a profile install is part of that run, a shared run's decline is the
//! caller's decline, and a write that panics releases the gate.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use systemprompt_bridge::elevation_gate::{Admission, ElevationGate, GateOutcome, Requester};

#[derive(Debug, PartialEq, Eq)]
enum WriteError {
    Declined,
    Failed,
}

fn write(
    gate: &ElevationGate,
    content: &str,
    run: impl FnOnce() -> Result<u32, WriteError>,
) -> Result<u32, WriteError> {
    gate.single_flight(
        content,
        run,
        |e| *e == WriteError::Declined,
        || WriteError::Declined,
    )
}

#[test]
fn distinct_writes_run_one_at_a_time_and_each_runs_its_own() {
    let gate = Arc::new(ElevationGate::new());
    let active = Arc::new(AtomicUsize::new(0));
    let runs = Arc::new(AtomicUsize::new(0));
    let (started, first_running) = mpsc::channel();
    let (release, hold) = mpsc::channel::<()>();
    let first = {
        let (gate, active, runs) = (Arc::clone(&gate), Arc::clone(&active), Arc::clone(&runs));
        std::thread::spawn(move || {
            write(&gate, "clear-mcp", || {
                assert_eq!(active.fetch_add(1, Ordering::SeqCst), 0);
                started.send(()).expect("signal");
                hold.recv().expect("release");
                runs.fetch_add(1, Ordering::SeqCst);
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(1)
            })
        })
    };
    first_running.recv().expect("the first write starts");
    let second = {
        let (gate, active, runs) = (Arc::clone(&gate), Arc::clone(&active), Arc::clone(&runs));
        std::thread::spawn(move || {
            write(&gate, "remove-profile", || {
                assert_eq!(
                    active.fetch_add(1, Ordering::SeqCst),
                    0,
                    "a second write never overlaps the first"
                );
                runs.fetch_add(1, Ordering::SeqCst);
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(2)
            })
        })
    };
    release.send(()).expect("release the first write");
    assert_eq!(first.join().unwrap(), Ok(1));
    assert_eq!(
        second.join().unwrap(),
        Ok(2),
        "a different write is never folded in"
    );
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[test]
fn a_write_waits_for_a_profile_install_in_flight() {
    let gate = Arc::new(ElevationGate::new());
    let Admission::Run(ticket) = gate.request("profile", Requester::Attended) else {
        panic!("an idle gate runs the profile install");
    };
    let profile_done = Arc::new(AtomicUsize::new(0));
    let writer = {
        let (gate, profile_done) = (Arc::clone(&gate), Arc::clone(&profile_done));
        std::thread::spawn(move || {
            write(&gate, "clear-mcp", || {
                assert_eq!(
                    profile_done.load(Ordering::SeqCst),
                    1,
                    "the write ran only after the profile install finished"
                );
                Ok(3)
            })
        })
    };
    profile_done.store(1, Ordering::SeqCst);
    ticket.finish(GateOutcome::Applied);
    assert_eq!(writer.join().unwrap(), Ok(3));
}

#[test]
fn a_write_from_the_thread_holding_a_profile_install_is_part_of_that_run() {
    let gate = ElevationGate::new();
    let Admission::Run(ticket) = gate.request("profile", Requester::Attended) else {
        panic!("an idle gate runs the profile install");
    };
    assert_eq!(
        write(&gate, "machine-policy", || Ok(4)),
        Ok(4),
        "the elevated write inside the install neither queues nor deadlocks"
    );
    ticket.finish(GateOutcome::Applied);
    assert_eq!(write(&gate, "after", || Ok(5)), Ok(5));
}

#[test]
fn a_declined_or_failed_write_is_the_callers_outcome_and_the_next_one_prompts() {
    let gate = ElevationGate::new();
    assert_eq!(
        write(&gate, "clear-mcp", || Err(WriteError::Declined)),
        Err(WriteError::Declined)
    );
    assert_eq!(
        write(&gate, "clear-mcp", || Err(WriteError::Failed)),
        Err(WriteError::Failed)
    );
    let prompts = AtomicUsize::new(0);
    assert_eq!(
        write(&gate, "clear-mcp", || {
            prompts.fetch_add(1, Ordering::SeqCst);
            Ok(6)
        }),
        Ok(6)
    );
    assert_eq!(
        prompts.load(Ordering::SeqCst),
        1,
        "a later request prompts again"
    );
}

#[test]
fn a_write_that_panics_releases_the_gate() {
    let gate = Arc::new(ElevationGate::new());
    let crashed = {
        let gate = Arc::clone(&gate);
        std::thread::spawn(move || write(&gate, "clear-mcp", || panic!("helper crashed")))
    };
    assert!(crashed.join().is_err());
    assert_eq!(write(&gate, "clear-mcp", || Ok(7)), Ok(7));
    let Admission::Run(ticket) = gate.request("profile", Requester::Attended) else {
        panic!("a profile install is not blocked by the crashed write");
    };
    ticket.finish(GateOutcome::Applied);
}
