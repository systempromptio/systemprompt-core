use systemprompt_harness::members::slack::is_slack_originated;

use crate::members::slack::support::notification;

#[test]
fn a_slack_kind_is_slack_originated() {
    assert!(is_slack_originated(&notification("slack.message", &[])));
    assert!(is_slack_originated(&notification("slack", &[])));
}

#[test]
fn a_slack_subject_is_slack_originated() {
    assert!(is_slack_originated(&notification(
        "stream.published",
        &["slack"]
    )));
    assert!(is_slack_originated(&notification(
        "stream.published",
        &["document", "slack.channel"]
    )));
}

#[test]
fn other_notifications_are_not() {
    assert!(!is_slack_originated(&notification(
        "document.shared",
        &["document"]
    )));
    assert!(!is_slack_originated(&notification(
        "slackbot.message",
        &["slackware"]
    )));
}
