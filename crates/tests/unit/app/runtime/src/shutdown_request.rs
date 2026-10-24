use std::time::Duration;

use systemprompt_runtime::ShutdownRequest;

#[tokio::test]
async fn request_wakes_a_waiter() {
    let request = ShutdownRequest::default();
    let waiter = request.clone();
    let task = tokio::spawn(async move { waiter.requested().await });

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

    let mut a = tokio::spawn(async move { first.requested().await });
    let mut b = tokio::spawn(async move { second.requested().await });

    request.request("once");

    let (woken, other) = tokio::select! {
        result = &mut a => (result, &mut b),
        result = &mut b => (result, &mut a),
    };
    assert!(woken.is_ok(), "the woken waiter panicked");
    let second_wake = tokio::time::timeout(Duration::from_millis(50), other).await;
    assert!(second_wake.is_err(), "a single request woke both waiters");
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
    let mut task = tokio::spawn(async move { waiter.requested().await });

    other.request("unrelated");

    let woken = tokio::time::timeout(Duration::from_millis(50), &mut task).await;
    assert!(
        woken.is_err(),
        "handles built separately must not share a notification"
    );
    task.abort();
}

#[tokio::test]
async fn a_clone_observes_a_request_raised_on_the_original() {
    let request = ShutdownRequest::default();
    let waiter = request.clone();
    let task = tokio::spawn(async move { waiter.requested().await });

    request.request("clone");

    let woken = tokio::time::timeout(Duration::from_secs(2), task).await;
    assert!(
        woken.is_ok(),
        "a clone must observe requests raised on the handle it came from"
    );
}
