use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use systemprompt_bridge::elevation_gate::{Admission, ElevationGate, GateOutcome, Requester};

fn run_prompt(
    gate: &ElevationGate,
    content: &str,
    prompts: &AtomicUsize,
    outcome: GateOutcome,
) -> GateOutcome {
    match gate.request(content, Requester::Attended) {
        Admission::Run(ticket) => {
            prompts.fetch_add(1, Ordering::SeqCst);
            ticket.finish(outcome);
            outcome
        },
        Admission::Joined(o) => o,
        Admission::Suppressed => GateOutcome::Declined,
        Admission::Queued => GateOutcome::Failed,
    }
}

fn staged(gate: &ElevationGate, content: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while gate.pending().as_deref() != Some(content) {
        assert!(Instant::now() < deadline, "{content} was never staged");
        std::thread::yield_now();
    }
}

#[test]
fn a_request_for_the_content_in_flight_takes_its_result_without_prompting() {
    let gate = Arc::new(ElevationGate::new());
    let prompts = Arc::new(AtomicUsize::new(0));
    let Admission::Run(ticket) = gate.request("policy-a", Requester::Attended) else {
        panic!("an idle gate runs the first request");
    };
    let waiter = {
        let (gate, prompts) = (Arc::clone(&gate), Arc::clone(&prompts));
        std::thread::spawn(move || run_prompt(&gate, "policy-a", &prompts, GateOutcome::Applied))
    };
    staged(&gate, "policy-a");
    ticket.finish(GateOutcome::Applied);
    assert_eq!(waiter.join().unwrap(), GateOutcome::Applied);
    assert_eq!(
        prompts.load(Ordering::SeqCst),
        0,
        "the waiter rode the run in flight"
    );
}

#[test]
fn changes_arriving_during_a_prompt_coalesce_into_one_more() {
    let gate = Arc::new(ElevationGate::new());
    let prompts = Arc::new(AtomicUsize::new(0));
    let Admission::Run(ticket) = gate.request("v1", Requester::Attended) else {
        panic!("an idle gate runs the first request");
    };
    let mut later = Vec::new();
    for content in ["v2", "v3"] {
        let (shared, prompts) = (Arc::clone(&gate), Arc::clone(&prompts));
        later.push(std::thread::spawn(move || {
            run_prompt(&shared, content, &prompts, GateOutcome::Applied)
        }));
        staged(&gate, content);
    }
    ticket.finish(GateOutcome::Applied);
    for h in later {
        assert_eq!(h.join().unwrap(), GateOutcome::Applied);
    }
    assert_eq!(
        prompts.load(Ordering::SeqCst),
        1,
        "both changes made during the first prompt ride one more"
    );
    assert!(gate.pending().is_none(), "the newest content was applied");
}

#[test]
fn a_waiter_is_not_answered_until_the_run_covering_it_finishes() {
    let gate = Arc::new(ElevationGate::new());
    let Admission::Run(first) = gate.request("v1", Requester::Attended) else {
        panic!("an idle gate runs the first request");
    };
    let answered = Arc::new(AtomicUsize::new(0));
    let waiter = {
        let (gate, answered) = (Arc::clone(&gate), Arc::clone(&answered));
        std::thread::spawn(move || {
            let admission = gate.request("v2", Requester::Attended);
            answered.fetch_add(1, Ordering::SeqCst);
            match admission {
                Admission::Run(ticket) => {
                    ticket.finish(GateOutcome::Applied);
                    GateOutcome::Applied
                },
                Admission::Joined(o) => o,
                Admission::Suppressed | Admission::Queued => GateOutcome::Failed,
            }
        })
    };
    staged(&gate, "v2");
    first.finish(GateOutcome::Declined);
    assert_eq!(
        waiter.join().unwrap(),
        GateOutcome::Applied,
        "v2 was staged after v1 started, so v1's decline is not its answer"
    );
    assert_eq!(answered.load(Ordering::SeqCst), 1);
}

#[test]
fn a_decline_suppresses_the_same_content_until_the_user_asks_again() {
    let gate = ElevationGate::new();
    let prompts = AtomicUsize::new(0);
    assert_eq!(
        run_prompt(&gate, "p", &prompts, GateOutcome::Declined),
        GateOutcome::Declined
    );
    assert!(matches!(
        gate.request("p", Requester::Attended),
        Admission::Suppressed
    ));
    assert!(
        matches!(
            gate.request("changed", Requester::Attended),
            Admission::Run(_)
        ),
        "changed content may ask again"
    );
    assert!(matches!(
        gate.request("p", Requester::UserAction),
        Admission::Run(_)
    ));
    assert_eq!(prompts.load(Ordering::SeqCst), 1);
}

#[test]
fn an_unattended_caller_never_prompts_and_leaves_the_change_pending() {
    let gate = ElevationGate::new();
    assert!(matches!(
        gate.request("p", Requester::Unattended),
        Admission::Queued
    ));
    assert_eq!(gate.pending().as_deref(), Some("p"));
}

#[test]
fn a_ticket_dropped_unfinished_releases_the_gate_as_failed() {
    let gate = ElevationGate::new();
    match gate.request("p", Requester::Attended) {
        Admission::Run(ticket) => drop(ticket),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        gate.request("p", Requester::Attended),
        Admission::Run(_)
    ));
}
