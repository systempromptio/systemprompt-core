use std::sync::Arc;

use systemprompt_extension::notifications::{ChannelError, NotificationChannel};
use systemprompt_models::devices::AppIdentifier;
use systemprompt_models::notifications::{
    Attention, CollapseKey, PushCategory, PushPayload, PushProvider,
};
use systemprompt_notifications::channels::apns::{ApnsChannel, ApnsSender};
use systemprompt_notifications::channels::fcm::{FcmChannel, FcmProjectId, FcmSender};
use systemprompt_notifications::channels::push::{
    PushChannel, PushError, PushSender, PushVerdict, category_of,
};
use systemprompt_notifications::plane::push::ApnsEnvironment;

use crate::fixtures::kind;
use crate::push_support::{
    APP, BEARER, BODY, StubCredential, StubDirectory, StubHttp, StubPresence, TITLE, device,
    device_token, notification,
};

const APNS_TOKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";
const FCM_TOKEN: &str = "fcm-registration-token:APA91b";
const APPROVAL_KIND: &str = "run.approval_requested";

fn apns_sender(http: &Arc<StubHttp>) -> ApnsSender {
    ApnsSender::new(
        Arc::clone(http) as _,
        Arc::new(StubCredential),
        ApnsEnvironment::Production,
        AppIdentifier::try_new(APP).expect("topic"),
    )
}

fn fcm_sender(http: &Arc<StubHttp>) -> FcmSender {
    FcmSender::new(
        Arc::clone(http) as _,
        Arc::new(StubCredential),
        FcmProjectId::try_new("example-project").expect("project"),
    )
}

fn apns_channel(http: &Arc<StubHttp>, directory: &Arc<StubDirectory>, live: bool) -> ApnsChannel {
    PushChannel::new(
        apns_sender(http),
        Arc::clone(directory) as _,
        Arc::new(StubPresence(live)),
    )
}

fn fcm_channel(http: &Arc<StubHttp>, directory: &Arc<StubDirectory>, live: bool) -> FcmChannel {
    PushChannel::new(
        fcm_sender(http),
        Arc::clone(directory) as _,
        Arc::new(StubPresence(live)),
    )
}

fn both_tokens() -> Arc<StubDirectory> {
    StubDirectory::holding(vec![
        device_token("phone", PushProvider::Apns, APNS_TOKEN, APP),
        device_token("tablet", PushProvider::Fcm, FCM_TOKEN, APP),
    ])
}

fn parsed(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("the provider body is JSON")
}

fn keys(value: &serde_json::Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

fn payload() -> PushPayload {
    let id = systemprompt_identifiers::NotificationId::generate();
    let collapse = CollapseKey::try_new(id.as_str()).expect("collapse key");
    PushPayload::new(id, PushCategory::Notification, collapse)
}

#[tokio::test]
async fn an_apns_push_carries_only_the_three_payload_fields() {
    let http = StubHttp::answering(200, "");
    let directory = both_tokens();
    let sent = notification(APPROVAL_KIND, Attention::Human);

    apns_channel(&http, &directory, false)
        .deliver(&sent, 3)
        .await
        .expect("the push is delivered");

    let requests = http.requests();
    assert_eq!(requests.len(), 1, "only the Apple token is pushed to");
    let request = &requests[0];
    assert_eq!(
        request.url,
        format!("https://api.push.apple.com/3/device/{APNS_TOKEN}")
    );
    assert_eq!(request.bearer.expose(), BEARER);
    let body = parsed(&request.body);
    assert_eq!(
        keys(&body),
        ["aps", "category", "collapse_key", "notification_id"]
    );
    assert_eq!(body["aps"], serde_json::json!({ "content-available": 1 }));
    assert_eq!(body["notification_id"], sent.id.as_str());
    assert_eq!(body["category"], "approval");
    assert_eq!(body["collapse_key"], sent.id.as_str());
    assert!(!request.body.contains(TITLE) && !request.body.contains(BODY));
    let headers: Vec<(&str, &str)> = request
        .headers
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    assert_eq!(
        headers,
        [
            ("apns-topic", APP),
            ("apns-push-type", "background"),
            ("apns-priority", "5"),
            ("apns-collapse-id", sent.id.as_str()),
        ]
    );
}

#[tokio::test]
async fn an_fcm_push_carries_only_the_three_payload_fields() {
    let http = StubHttp::answering(200, "{}");
    let directory = both_tokens();
    let sent = notification("document.shared", Attention::Human);

    fcm_channel(&http, &directory, false)
        .deliver(&sent, 0)
        .await
        .expect("the push is delivered");

    let requests = http.requests();
    assert_eq!(requests.len(), 1, "only the Google token is pushed to");
    let request = &requests[0];
    assert_eq!(
        request.url,
        "https://fcm.googleapis.com/v1/projects/example-project/messages:send"
    );
    let body = parsed(&request.body);
    assert_eq!(keys(&body), ["message"]);
    let message = &body["message"];
    assert_eq!(keys(message), ["android", "data", "token"]);
    assert_eq!(message["token"], FCM_TOKEN);
    assert_eq!(
        keys(&message["data"]),
        ["category", "collapse_key", "notification_id"]
    );
    assert_eq!(message["data"]["notification_id"], sent.id.as_str());
    assert_eq!(message["data"]["category"], "notification");
    assert_eq!(
        message["android"],
        serde_json::json!({ "collapse_key": sent.id.as_str(), "priority": "HIGH" })
    );
    assert!(!request.body.contains(TITLE) && !request.body.contains(BODY));
}

#[tokio::test]
async fn an_unregistered_apns_token_is_revoked_in_the_delivery() {
    for (status, body) in [
        (410, r#"{"reason":"Unregistered"}"#),
        (410, r#"{"reason":"ExpiredToken"}"#),
    ] {
        let http = StubHttp::answering(status, body);
        let directory = both_tokens();

        apns_channel(&http, &directory, false)
            .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
            .await
            .expect("an unregistered token is not a delivery failure");

        assert_eq!(directory.revoked(), [(device("phone"), PushProvider::Apns)]);
    }
}

#[tokio::test]
async fn a_bad_device_token_from_apns_is_a_refusal_that_revokes_nothing() {
    let http = StubHttp::answering(400, r#"{"reason":"BadDeviceToken"}"#);
    let directory = both_tokens();

    apns_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect_err("a token the environment does not recognise is a refused delivery");

    assert!(directory.revoked().is_empty());
}

#[tokio::test]
async fn an_unregistered_fcm_token_is_revoked_in_the_delivery() {
    let unregistered = r#"{"error":{"code":404,"status":"NOT_FOUND","details":[{"@type":"type.googleapis.com/google.firebase.fcm.v1.FcmError","errorCode":"UNREGISTERED"}]}}"#;
    let http = StubHttp::answering(404, unregistered);
    let directory = both_tokens();

    fcm_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect("an unregistered token is not a delivery failure");

    assert_eq!(directory.revoked(), [(device("tablet"), PushProvider::Fcm)]);
}

#[tokio::test]
async fn the_revocation_names_the_token_the_provider_refused() {
    let unregistered = r#"{"error":{"code":404,"status":"NOT_FOUND","details":[{"@type":"type.googleapis.com/google.firebase.fcm.v1.FcmError","errorCode":"UNREGISTERED"}]}}"#;
    let http = StubHttp::answering(404, unregistered);
    let directory = both_tokens();

    fcm_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect("an unregistered token is not a delivery failure");

    assert_eq!(
        directory.revoked_tokens(),
        [device_token("tablet", PushProvider::Fcm, FCM_TOKEN, APP)],
        "the directory drops the refused value, so a token registered since is kept"
    );
}

#[tokio::test]
async fn an_accepted_push_revokes_nothing() {
    let http = StubHttp::answering(200, "");
    let directory = both_tokens();

    apns_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect("the push is delivered");

    assert!(directory.revoked().is_empty());
}

#[tokio::test]
async fn a_recipient_with_a_live_stream_is_not_pushed_to() {
    let http = StubHttp::answering(200, "");
    let directory = both_tokens();
    let sent = notification(APPROVAL_KIND, Attention::Human);

    apns_channel(&http, &directory, true)
        .deliver(&sent, 0)
        .await
        .expect("suppression is not a failure");
    fcm_channel(&http, &directory, true)
        .deliver(&sent, 0)
        .await
        .expect("suppression is not a failure");

    assert!(http.requests().is_empty());
}

#[test]
fn only_human_attention_warrants_a_push() {
    let http = StubHttp::answering(200, "");
    let directory = both_tokens();
    let channel = apns_channel(&http, &directory, false);

    assert!(channel.delivers(Attention::Human));
    assert!(!channel.delivers(Attention::Agent));
    assert!(!channel.delivers(Attention::Silent));
    assert_eq!(channel.surface(), "apns");
    assert_eq!(fcm_channel(&http, &directory, false).surface(), "fcm");
}

#[tokio::test]
async fn an_apns_token_for_another_app_is_not_pushed_to() {
    let http = StubHttp::answering(200, "");
    let directory = StubDirectory::holding(vec![device_token(
        "phone",
        PushProvider::Apns,
        APNS_TOKEN,
        "io.example.other",
    )]);

    apns_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect("no token to push to is not a failure");

    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_stored_apns_token_that_is_not_hexadecimal_is_revoked_unsent() {
    let http = StubHttp::answering(200, "");
    let directory = StubDirectory::holding(vec![device_token(
        "phone",
        PushProvider::Apns,
        "../../3/device/other",
        APP,
    )]);

    apns_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect("the token is dropped");

    assert!(http.requests().is_empty());
    assert_eq!(directory.revoked(), [(device("phone"), PushProvider::Apns)]);
}

#[tokio::test]
async fn apns_answers_are_classified_retryable_or_not() {
    let token = device_token("phone", PushProvider::Apns, APNS_TOKEN, APP);
    for (status, body, retryable) in [
        (503, r#"{"reason":"ServiceUnavailable"}"#, true),
        (429, r#"{"reason":"TooManyRequests"}"#, true),
        (500, "", true),
        (403, r#"{"reason":"ExpiredProviderToken"}"#, true),
        (403, r#"{"reason":"InvalidProviderToken"}"#, false),
        (400, r#"{"reason":"BadTopic"}"#, false),
        (400, r#"{"reason":"DeviceTokenNotForTopic"}"#, false),
    ] {
        let http = StubHttp::answering(status, body);
        let error = apns_sender(&http)
            .send(&token, &payload())
            .await
            .expect_err("the provider refused");
        assert_eq!(error.is_retryable(), retryable, "{status} {body}");
    }
}

#[tokio::test]
async fn fcm_answers_are_classified_retryable_or_not() {
    let token = device_token("tablet", PushProvider::Fcm, FCM_TOKEN, APP);
    for (status, body, retryable) in [
        (503, r#"{"error":{"status":"UNAVAILABLE"}}"#, true),
        (429, r#"{"error":{"status":"RESOURCE_EXHAUSTED"}}"#, true),
        (500, r#"{"error":{"status":"INTERNAL"}}"#, true),
        (401, r#"{"error":{"status":"UNAUTHENTICATED"}}"#, true),
        (400, r#"{"error":{"status":"INVALID_ARGUMENT"}}"#, false),
        (
            403,
            r#"{"error":{"status":"PERMISSION_DENIED","details":[{"errorCode":"SENDER_ID_MISMATCH"}]}}"#,
            false,
        ),
    ] {
        let http = StubHttp::answering(status, body);
        let error = fcm_sender(&http)
            .send(&token, &payload())
            .await
            .expect_err("the provider refused");
        assert_eq!(error.is_retryable(), retryable, "{status} {body}");
    }
}

#[tokio::test]
async fn an_accepted_answer_is_an_accepted_verdict() {
    let http = StubHttp::answering(200, r#"{"name":"projects/example-project/messages/1"}"#);
    let verdict = fcm_sender(&http)
        .send(
            &device_token("tablet", PushProvider::Fcm, FCM_TOKEN, APP),
            &payload(),
        )
        .await
        .expect("accepted");
    assert_eq!(verdict, PushVerdict::Accepted);
}

#[tokio::test]
async fn a_provider_refusal_fails_the_channel_with_its_cause() {
    let http = StubHttp::answering(503, r#"{"reason":"ServiceUnavailable"}"#);
    let directory = both_tokens();

    let error = apns_channel(&http, &directory, false)
        .deliver(&notification(APPROVAL_KIND, Attention::Human), 0)
        .await
        .expect_err("the provider is unavailable");

    assert!(error.is_retryable());
    let ChannelError::Other { surface, source } = error else {
        panic!("a provider failure is a channel failure with a source");
    };
    assert_eq!(surface, "apns");
    let cause = source
        .downcast_ref::<PushError>()
        .expect("the push error is the source");
    assert!(matches!(cause, PushError::Unavailable { status: 503, .. }));
    assert!(directory.revoked().is_empty());
}

#[test]
fn only_an_approval_request_is_in_the_approval_category() {
    assert_eq!(category_of(&kind(APPROVAL_KIND)), PushCategory::Approval);
    assert_eq!(
        category_of(&kind("session.turn.approval_requested")),
        PushCategory::Approval
    );
    assert_eq!(
        category_of(&kind("document.shared")),
        PushCategory::Notification
    );
    assert_eq!(
        category_of(&kind("run.approval_decided")),
        PushCategory::Notification
    );
}
