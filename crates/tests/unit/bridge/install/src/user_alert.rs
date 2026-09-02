use std::time::{Duration, Instant};

use systemprompt_bridge::user_alert::alert_user;

// The alert is raised from installer paths that have no window and no return
// channel, so the only contract it can break is blocking the caller: on a host
// with no notification daemon the spawn must fail fast and return, never wait
// on a dialog nobody can dismiss.
#[test]
fn alerting_returns_promptly_when_no_notifier_is_available() {
    let started = Instant::now();
    alert_user("Bridge needs attention", "Approve the managed profile.");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "alert_user must not block the caller; took {elapsed:?}"
    );
}

// Quote characters are stripped rather than escaped because the
// shell/AppleScript commands are quote-delimited; adversarial text must still
// return cleanly.
#[test]
fn quote_laden_text_does_not_derail_the_alert() {
    let started = Instant::now();
    alert_user(
        "a \"quoted\" title with 'both' kinds",
        "body with \" and ' and a trailing backslash \\",
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "quote stripping must keep the command well-formed; took {elapsed:?}"
    );
}
