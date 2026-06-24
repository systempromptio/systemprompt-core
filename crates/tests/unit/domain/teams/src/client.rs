//! Tests for the outbound reply-URL builder.

use systemprompt_identifiers::TeamsConversationId;
use systemprompt_teams::client::reply_url;

#[test]
fn builds_the_bot_connector_reply_endpoint() {
    let url = reply_url(
        "https://smba.trafficmanager.net/uk/",
        &TeamsConversationId::new("19:abc@thread.v2"),
    );
    assert_eq!(
        url,
        "https://smba.trafficmanager.net/uk/v3/conversations/19:abc@thread.v2/activities"
    );
}

#[test]
fn trims_a_single_trailing_slash_on_the_service_url() {
    let no_slash = reply_url("https://smba.example", &TeamsConversationId::new("c"));
    let with_slash = reply_url("https://smba.example/", &TeamsConversationId::new("c"));
    assert_eq!(no_slash, with_slash);
    assert_eq!(
        no_slash,
        "https://smba.example/v3/conversations/c/activities"
    );
}
