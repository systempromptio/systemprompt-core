//! Which notifications came from Slack.
//!
//! A notification names no delivery surface, so the rule reads what the
//! notification does carry: it is Slack-originated when the first segment of
//! its kind is `slack` (an extension kind such as `slack.message`), or when
//! one of its subjects is a resource whose kind's first segment is `slack`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_notifications::Notification;

const SLACK: &str = "slack";

pub fn is_slack_originated(notification: &Notification) -> bool {
    first_segment(notification.kind.as_str()) == SLACK
        || notification
            .subjects
            .iter()
            .any(|subject| first_segment(&subject.kind) == SLACK)
}

fn first_segment(value: &str) -> &str {
    value.split_once('.').map_or(value, |(head, _)| head)
}
