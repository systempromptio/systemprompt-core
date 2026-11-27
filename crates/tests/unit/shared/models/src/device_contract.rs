use chrono::{TimeZone, Utc};
use semver::Version;
use serde_json::json;
use systemprompt_identifiers::{DeviceId, UserId};
use systemprompt_models::devices::{
    AdminDeviceRow, AppIdentifier, DeviceClientInfo, DevicePlatform,
};
use systemprompt_models::notifications::PushProvider;

#[test]
fn every_platform_round_trips_through_its_wire_form() {
    for platform in DevicePlatform::ALL {
        assert_eq!(platform.as_str().parse::<DevicePlatform>(), Ok(platform));
        let json = serde_json::to_string(&platform).unwrap();
        assert_eq!(json, format!("\"{}\"", platform.as_str()));
        assert_eq!(
            serde_json::from_str::<DevicePlatform>(&json).unwrap(),
            platform
        );
        assert_eq!(platform.to_string(), platform.as_str());
    }
}

#[test]
fn the_wire_forms_are_the_six_lowercase_names() {
    assert_eq!(
        DevicePlatform::ALL.map(DevicePlatform::as_str),
        ["ios", "android", "macos", "windows", "linux", "unknown"]
    );
}

#[test]
fn a_record_with_no_platform_is_unknown() {
    assert_eq!(DevicePlatform::default(), DevicePlatform::Unknown);
    assert!("iOS".parse::<DevicePlatform>().is_err());
    assert!("".parse::<DevicePlatform>().is_err());
}

#[test]
fn only_the_phone_platforms_have_a_push_provider() {
    assert_eq!(
        DevicePlatform::Ios.push_provider(),
        Some(PushProvider::Apns)
    );
    assert_eq!(
        DevicePlatform::Android.push_provider(),
        Some(PushProvider::Fcm)
    );
    for platform in DevicePlatform::ALL {
        assert_eq!(platform.is_phone(), platform.push_provider().is_some());
    }
    for provider in PushProvider::ALL {
        assert_eq!(provider.platform().push_provider(), Some(provider));
    }
}

#[test]
fn an_app_identifier_is_a_reverse_dns_name() {
    for valid in ["io.example.app", "com.example.my-app", "a.b_c.D9"] {
        assert_eq!(AppIdentifier::try_new(valid).unwrap().as_str(), valid);
    }
    for invalid in [
        "",
        "app",
        "io..app",
        ".io.app",
        "io.app.",
        "io.exa mple",
        "io.example/app",
    ] {
        assert!(AppIdentifier::try_new(invalid).is_err(), "{invalid}");
    }
    assert!(AppIdentifier::try_new(format!("a.{}", "b".repeat(254))).is_err());
    assert!(serde_json::from_value::<AppIdentifier>(json!("app")).is_err());
}

#[test]
fn client_info_refuses_an_unknown_key_and_a_non_semver_app_version() {
    let body = json!({
        "platform": "ios",
        "os_version": "18.1",
        "app_version": "1.4.0",
        "app_id": "io.example.app",
    });
    let info: DeviceClientInfo = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(info.platform, DevicePlatform::Ios);
    assert_eq!(info.app_version, Version::new(1, 4, 0));
    assert_eq!(serde_json::to_value(&info).unwrap(), body);

    let mut extra = body.clone();
    extra["label"] = json!("x");
    assert!(serde_json::from_value::<DeviceClientInfo>(extra).is_err());
    let mut loose = body;
    loose["app_version"] = json!("1.4");
    assert!(serde_json::from_value::<DeviceClientInfo>(loose).is_err());
}

#[test]
fn an_admin_row_of_a_record_that_reported_nothing_is_unknown_with_no_versions() {
    let enrolled_at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    let row = AdminDeviceRow {
        device_id: DeviceId::try_new("dev_1").unwrap(),
        user_id: UserId::new("user_1"),
        label: "Laptop".to_owned(),
        platform: DevicePlatform::Unknown,
        os_version: None,
        app_version: None,
        app_id: None,
        push_provider: None,
        enrolled_at,
        last_seen_at: None,
        revoked_at: None,
    };
    let value = serde_json::to_value(&row).unwrap();
    assert_eq!(
        value,
        json!({
            "device_id": "dev_1",
            "user_id": "user_1",
            "label": "Laptop",
            "platform": "unknown",
            "enrolled_at": "2026-01-02T03:04:05Z",
        })
    );
    assert_eq!(
        serde_json::from_value::<AdminDeviceRow>(value).unwrap(),
        row
    );
}

#[test]
fn an_admin_row_of_a_phone_shows_platform_and_app_version() {
    let at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    let row = AdminDeviceRow {
        device_id: DeviceId::try_new("dev_2").unwrap(),
        user_id: UserId::new("user_1"),
        label: "Phone".to_owned(),
        platform: DevicePlatform::Android,
        os_version: Some("15".to_owned()),
        app_version: Some(Version::new(1, 4, 0)),
        app_id: Some(AppIdentifier::try_new("io.example.app").unwrap()),
        push_provider: Some(PushProvider::Fcm),
        enrolled_at: at,
        last_seen_at: Some(at),
        revoked_at: None,
    };
    let value = serde_json::to_value(&row).unwrap();
    assert_eq!(value["platform"], "android");
    assert_eq!(value["app_version"], "1.4.0");
    assert_eq!(value["app_id"], "io.example.app");
    assert_eq!(value["push_provider"], "fcm");
    assert!(value.get("revoked_at").is_none());
}
