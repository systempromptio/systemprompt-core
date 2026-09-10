use std::time::Duration;

use systemprompt_runtime::ShutdownRequest;

#[tokio::test]
async fn request_wakes_a_waiter() {
    let request = ShutdownRequest::default();
    let waiter = request.clone();
    let task = tokio::spawn(async move { waiter.requested().await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    request.request("test");

    let woken = tokio::time::timeout(Duration::from_secs(2), task).await;
    assert!(woken.is_ok(), "waiter did not wake within the timeout");
    assert!(woken.expect("timeout").is_ok(), "waiter task panicked");
}

#[tokio::test]
async fn request_raised_before_anyone_waits_is_delivered() {
    let request = ShutdownRequest::default();
    request.request("early");

    let delivered = tokio::time::timeout(Duration::from_secs(2), request.requested()).await;
    assert!(delivered.is_ok(), "stored request was never delivered");
}

#[tokio::test]
async fn one_request_wakes_exactly_one_waiter() {
    let request = ShutdownRequest::default();
    let first = request.clone();
    let second = request.clone();

    let a = tokio::spawn(async move { first.requested().await });
    let b = tokio::spawn(async move { second.requested().await });
    tokio::time::sleep(Duration::from_millis(20)).await;

    request.request("once");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let finished = usize::from(a.is_finished()) + usize::from(b.is_finished());
    assert_eq!(finished, 1, "a single request woke {finished} waiters");
    a.abort();
    b.abort();
}

#[tokio::test]
async fn a_select_resolves_on_the_request_branch() {
    let request = ShutdownRequest::default();
    let waiter = request.clone();
    request.request("select");

    let resolved = tokio::select! {
        () = waiter.requested() => "restart",
        () = tokio::time::sleep(Duration::from_secs(2)) => "timeout",
    };

    assert_eq!(resolved, "restart");
}

#[tokio::test]
async fn a_separate_handle_does_not_signal_an_unrelated_waiter() {
    let other = ShutdownRequest::default();
    let waiter = ShutdownRequest::default();
    let task = tokio::spawn(async move { waiter.requested().await });
    tokio::time::sleep(Duration::from_millis(20)).await;

    other.request("unrelated");
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !task.is_finished(),
        "handles built separately must not share a notification"
    );
    task.abort();
}

#[tokio::test]
async fn a_clone_observes_a_request_raised_on_the_original() {
    let request = ShutdownRequest::default();
    let waiter = request.clone();
    let task = tokio::spawn(async move { waiter.requested().await });
    tokio::time::sleep(Duration::from_millis(20)).await;

    request.request("clone");

    let woken = tokio::time::timeout(Duration::from_secs(2), task).await;
    assert!(
        woken.is_ok(),
        "a clone must observe requests raised on the handle it came from"
    );
}
