use std::time::Duration;

use harness_tasks::{CancellationToken, OwnedThread, TaskError};

#[test]
fn stop_cancels_the_thread_and_hands_back_its_value() {
    let thread = OwnedThread::spawn("counter", CancellationToken::new(), |token| {
        let mut ticks = 0u32;
        while !token.is_cancelled() {
            ticks += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        ticks
    })
    .unwrap();
    assert_eq!(thread.name(), "counter");
    std::thread::sleep(Duration::from_millis(10));
    assert!(!thread.is_finished());
    assert!(thread.stop().unwrap() > 0);
}

#[test]
fn a_parent_token_stops_the_thread() {
    let parent = CancellationToken::new();
    let thread = OwnedThread::spawn("child", parent.child_token(), |token| {
        while !token.is_cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        "stopped"
    })
    .unwrap();
    parent.cancel();
    assert_eq!(thread.join().unwrap(), "stopped");
}

#[test]
fn join_reports_a_panicking_thread() {
    let thread = OwnedThread::spawn("doomed", CancellationToken::new(), |_token| -> u8 {
        panic!("boom");
    })
    .unwrap();
    let error = thread.join().unwrap_err();
    assert!(matches!(
        error,
        TaskError::ThreadPanicked { name: "doomed" }
    ));
}

#[test]
fn dropping_the_thread_cancels_its_token() {
    let token = CancellationToken::new();
    let observed = token.clone();
    let thread = OwnedThread::spawn("watcher", token, |token| {
        while !token.is_cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
    })
    .unwrap();
    drop(thread);
    assert!(observed.is_cancelled());
}
