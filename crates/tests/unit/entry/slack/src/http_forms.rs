//! Unit coverage for the Slack route's form helpers: `parse_form` and
//! `slash_command_from_form` are public in the route module and touch neither
//! the network nor the database.

use systemprompt_slack_entry::http::{parse_form, slash_command_from_form};

#[test]
fn parse_form_decodes_url_encoded_pairs() {
    let form = parse_form(b"command=%2Fask&text=hello+world&team_id=T1");
    assert_eq!(form.get("command").map(String::as_str), Some("/ask"));
    assert_eq!(form.get("text").map(String::as_str), Some("hello world"));
    assert_eq!(form.get("team_id").map(String::as_str), Some("T1"));
}

#[test]
fn slash_command_from_a_complete_form_parses() {
    let form = parse_form(
        b"command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id=T1&response_url=https%3A%2F%2Fhooks.slack.com%2Fr",
    );
    let cmd = slash_command_from_form(&form).expect("a complete form yields a slash command");
    assert_eq!(cmd.team_id.as_str(), "T1");
    assert_eq!(cmd.command, "/ask");
}

#[test]
fn slash_command_missing_a_required_field_is_none() {
    // No `response_url` — `slash_command_from_form` requires it.
    let form = parse_form(b"command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id=T1");
    assert!(slash_command_from_form(&form).is_none());
}
