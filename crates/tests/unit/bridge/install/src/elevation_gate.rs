use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
            std::thread::sleep(Duration::from_millis(80));
            ticket.finish(outcome);
            outcome
        },
        Admission::Joined(o) => o,
        Admission::Suppressed => GateOutcome::Declined,
        Admission::Queued => GateOutcome::Failed,
    }
}

#[test]
fn concurrent_requests_for_the_same_policy_share_one_prompt() {
    let gate = Arc::new(ElevationGate::new());
    let prompts = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let gate = Arc::clone(&gate);
            let prompts = Arc::clone(&prompts);
            std::thread::spawn(move || {
                run_prompt(&gate, "policy-a", &prompts, GateOutcome::Applied)
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), GateOutcome::Applied);
    }
    assert_eq!(
        prompts.load(Ordering::SeqCst),
        1,
        "four callers, one approval"
    );
}

#[test]
fn changes_arriving_during_a_prompt_coalesce_into_one_more() {
    let gate = Arc::new(ElevationGate::new());
    let prompts = Arc::new(AtomicUsize::new(0));
    let first = {
        let (gate, prompts) = (Arc::clone(&gate), Arc::clone(&prompts));
        std::thread::spawn(move || run_prompt(&gate, "v1", &prompts, GateOutcome::Applied))
    };
    std::thread::sleep(Duration::from_millis(20));
    let later: Vec<_> = ["v2", "v3", "v3"]
        .into_iter()
        .map(|c| {
            let (gate, prompts) = (Arc::clone(&gate), Arc::clone(&prompts));
            std::thread::spawn(move || run_prompt(&gate, c, &prompts, GateOutcome::Applied))
        })
        .collect();
    first.join().unwrap();
    for h in later {
        h.join().unwrap();
    }
    assert!(
        prompts.load(Ordering::SeqCst) <= 2,
        "every change made during the first prompt rides one more, got {}",
        prompts.load(Ordering::SeqCst)
    );
    assert!(gate.pending().is_none(), "the newest content was applied");
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
