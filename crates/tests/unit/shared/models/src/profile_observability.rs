//! `observability.otlp`: the parsed shape, the defaults, the validator's
//! refusals, and the per-signal URL the exporter posts to.

use serde_yaml::Value;
use systemprompt_models::Profile;
use systemprompt_models::profile::{OtlpExportConfig, OtlpProtocol, OtlpSignal};

use crate::profile_services_sources::local_profile;

fn with_observability(block: &str) -> Result<Profile, serde_yaml::Error> {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    let block: Value = serde_yaml::from_str(block).expect("observability yaml");
    map.insert(Value::String("observability".to_owned()), block);
    serde_yaml::from_value(Value::Mapping(map))
}

fn errors_of(profile: &Profile) -> String {
    profile
        .validate()
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default()
}

#[test]
fn absent_block_means_no_export() {
    let profile = local_profile();
    assert!(profile.observability.otlp().is_none());
    assert!(!errors_of(&profile).contains("observability"));
}

#[test]
fn minimal_block_takes_the_defaults() {
    let profile =
        with_observability("otlp:\n  endpoint: https://collector.example:4318").expect("parses");
    let otlp = profile.observability.otlp().expect("otlp configured");
    assert_eq!(otlp.protocol, OtlpProtocol::Http);
    assert_eq!(otlp.signals, vec![OtlpSignal::Traces, OtlpSignal::Logs]);
    assert_eq!(otlp.batch_seconds, OtlpExportConfig::DEFAULT_BATCH_SECONDS);
    assert!(otlp.headers.is_empty());
    assert!(errors_of(&profile).is_empty(), "{}", errors_of(&profile));
}

#[test]
fn full_block_parses() {
    let profile = with_observability(
        "otlp:\n  endpoint: https://otlp.example\n  protocol: http\n  headers:\n    \
         DD-API-KEY: abc\n  signals: [traces]\n  batch_seconds: 60",
    )
    .expect("parses");
    let otlp = profile.observability.otlp().expect("otlp configured");
    assert_eq!(
        otlp.headers.get("DD-API-KEY").map(String::as_str),
        Some("abc")
    );
    assert!(otlp.exports(OtlpSignal::Traces));
    assert!(!otlp.exports(OtlpSignal::Logs));
    assert_eq!(otlp.batch_seconds, 60);
    assert!(errors_of(&profile).is_empty());
}

#[test]
fn unknown_key_is_rejected() {
    let err = with_observability("otlp:\n  endpoint: https://x\n  timeout: 3")
        .expect_err("unknown key rejected");
    assert!(err.to_string().contains("timeout"), "{err}");
}

#[test]
fn endpoint_must_be_an_http_url() {
    let profile = with_observability("otlp:\n  endpoint: collector:4318").expect("parses");
    assert!(errors_of(&profile).contains("observability.otlp.endpoint"));
    let profile = with_observability("otlp:\n  endpoint: ftp://collector").expect("parses");
    assert!(errors_of(&profile).contains("must be an http:// or https:// URL"));
}

#[test]
fn grpc_is_refused_as_not_yet_supported() {
    let profile = with_observability("otlp:\n  endpoint: http://collector:4317\n  protocol: grpc")
        .expect("parses");
    let errors = errors_of(&profile);
    assert!(errors.contains("'grpc' is not yet supported"), "{errors}");
}

#[test]
fn signals_must_be_non_empty_and_distinct() {
    let profile = with_observability("otlp:\n  endpoint: http://c\n  signals: []").expect("parses");
    assert!(errors_of(&profile).contains("at least one of: traces, logs"));
    let profile =
        with_observability("otlp:\n  endpoint: http://c\n  signals: [logs, logs]").expect("parses");
    assert!(errors_of(&profile).contains("lists 'logs' more than once"));
    let err = with_observability("otlp:\n  endpoint: http://c\n  signals: [metrics]")
        .expect_err("metrics is not an export signal");
    assert!(err.to_string().contains("metrics"), "{err}");
}

#[test]
fn batch_seconds_is_bounded() {
    for value in ["0", "3601"] {
        let profile = with_observability(&format!(
            "otlp:\n  endpoint: http://c\n  batch_seconds: {value}"
        ))
        .expect("parses");
        assert!(
            errors_of(&profile).contains("batch_seconds must be between 1 and 3600"),
            "{value}: {}",
            errors_of(&profile)
        );
    }
}

#[test]
fn header_names_and_values_are_checked() {
    let profile = with_observability(
        "otlp:\n  endpoint: http://c\n  headers:\n    \"bad name\": v\n    ok: \"a\\nb\"",
    )
    .expect("parses");
    let errors = errors_of(&profile);
    assert!(errors.contains("invalid header name"), "{errors}");
    assert!(errors.contains("must not contain line breaks"), "{errors}");
}

#[test]
fn signal_url_appends_the_otlp_path_once() {
    let mut otlp = OtlpExportConfig {
        endpoint: "https://collector.example:4318/".to_owned(),
        protocol: OtlpProtocol::Http,
        headers: Default::default(),
        signals: vec![OtlpSignal::Traces],
        batch_seconds: 15,
    };
    assert_eq!(
        otlp.signal_url(OtlpSignal::Traces),
        "https://collector.example:4318/v1/traces"
    );
    assert_eq!(
        otlp.signal_url(OtlpSignal::Logs),
        "https://collector.example:4318/v1/logs"
    );
    otlp.endpoint = "https://collector.example/v1/traces".to_owned();
    assert_eq!(
        otlp.signal_url(OtlpSignal::Traces),
        "https://collector.example/v1/traces"
    );
}

#[test]
fn signal_labels_round_trip() {
    for signal in OtlpSignal::ALL {
        assert_eq!(OtlpSignal::parse(signal.label()), Some(signal));
        assert_eq!(signal.to_string(), signal.label());
    }
    assert_eq!(OtlpSignal::parse("metrics"), None);
}
