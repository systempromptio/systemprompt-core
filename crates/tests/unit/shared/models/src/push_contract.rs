use serde_json::json;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{
    CollapseKey, PushCategory, PushPayload, PushProvider, PushToken, PushTokenError,
    RegisterPushTokenRequest,
};

const TOKEN: &str = "fGh1:APA91bH-secret_device_token_0123456789";

#[test]
fn a_payload_is_exactly_an_id_a_category_and_a_collapse_key() {
    let id = "7d3f2a34-93c1-4a55-9c0e-2f6f6a1f0b11";
    let payload = PushPayload::new(
        NotificationId::new(id),
        PushCategory::Approval,
        CollapseKey::try_new("approval:7d3f2a34").unwrap(),
    );
    let value = serde_json::to_value(&payload).unwrap();
    assert_eq!(
        value,
        json!({
            "notification_id": id,
            "category": "approval",
            "collapse_key": "approval:7d3f2a34",
        })
    );
    assert_eq!(
        serde_json::from_value::<PushPayload>(value).unwrap(),
        payload
    );
}

#[test]
fn a_payload_refuses_a_title_or_a_body() {
    for extra in ["title", "body"] {
        let mut value = json!({
            "notification_id": "7d3f2a34-93c1-4a55-9c0e-2f6f6a1f0b11",
            "category": "notification",
            "collapse_key": "k",
        });
        value[extra] = json!("An approval is waiting");
        assert!(
            serde_json::from_value::<PushPayload>(value).is_err(),
            "{extra}"
        );
    }
}

#[test]
fn the_categories_are_approval_and_notification() {
    assert_eq!(PushCategory::Approval.as_str(), "approval");
    assert_eq!(PushCategory::Notification.as_str(), "notification");
    assert_eq!(
        serde_json::to_value(PushCategory::Notification).unwrap(),
        json!("notification")
    );
    assert!(serde_json::from_value::<PushCategory>(json!("marketing")).is_err());
}

#[test]
fn a_collapse_key_is_bounded_ascii() {
    assert!(CollapseKey::try_new("a".repeat(64)).is_ok());
    assert!(CollapseKey::try_new("a".repeat(65)).is_err());
    assert!(CollapseKey::try_new("").is_err());
    assert!(CollapseKey::try_new("run 1").is_err());
    assert!(CollapseKey::try_new("run/1").is_err());
    assert!(serde_json::from_value::<CollapseKey>(json!("")).is_err());
}

#[test]
fn the_providers_are_apns_and_fcm() {
    assert_eq!(PushProvider::ALL.map(PushProvider::as_str), ["apns", "fcm"]);
    for provider in PushProvider::ALL {
        assert_eq!(provider.as_str().parse::<PushProvider>(), Ok(provider));
        assert_eq!(
            serde_json::to_value(provider).unwrap(),
            json!(provider.as_str())
        );
    }
    assert!("APNS".parse::<PushProvider>().is_err());
    assert!(serde_json::from_value::<PushProvider>(json!("webpush")).is_err());
}

#[test]
fn a_token_never_appears_in_debug_output() {
    let token = PushToken::try_new(TOKEN).unwrap();
    assert_eq!(token.expose(), TOKEN);
    let debug = format!("{token:?}");
    assert_eq!(debug, "PushToken(<redacted>)");

    let request: RegisterPushTokenRequest = serde_json::from_value(register_body()).unwrap();
    let debug = format!("{request:#?}");
    assert!(!debug.contains(TOKEN));
    assert!(!debug.contains("secret"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn a_token_is_validated_without_echoing_it() {
    assert_eq!(PushToken::try_new("").unwrap_err(), PushTokenError::Empty);
    assert_eq!(
        PushToken::try_new("a".repeat(4097)).unwrap_err(),
        PushTokenError::TooLong {
            len: 4097,
            max: 4096
        }
    );
    assert!(PushToken::try_new("a".repeat(4096)).is_ok());
    for invalid in ["with space", "line\nbreak", "tokén"] {
        let err = PushToken::try_new(invalid).unwrap_err();
        assert_eq!(err, PushTokenError::InvalidCharacter);
        assert!(!err.to_string().contains(invalid));
    }
    assert!(serde_json::from_value::<PushToken>(json!("")).is_err());
}

fn register_body() -> serde_json::Value {
    json!({
        "provider": "fcm",
        "token": TOKEN,
        "client": {
            "platform": "android",
            "os_version": "15",
            "app_version": "1.4.0",
            "app_id": "io.example.app",
        },
    })
}

#[test]
fn a_register_request_round_trips_and_names_no_device() {
    let request: RegisterPushTokenRequest = serde_json::from_value(register_body()).unwrap();
    assert_eq!(request.provider, PushProvider::Fcm);
    assert_eq!(request.token.expose(), TOKEN);
    assert_eq!(serde_json::to_value(&request).unwrap(), register_body());

    let mut targeted = register_body();
    targeted["device_id"] = json!("dev_other");
    assert!(serde_json::from_value::<RegisterPushTokenRequest>(targeted).is_err());
}
