use systemprompt_logging::{ExportCapability, OtlpExportUnavailable};
use systemprompt_manifest::profile::{OtlpExportConfig, OtlpProtocol, OtlpSignal};

fn collector() -> OtlpExportConfig {
    OtlpExportConfig {
        endpoint: "https://collector.example:4318".to_owned(),
        protocol: OtlpProtocol::Http,
        headers: Default::default(),
        signals: vec![OtlpSignal::Traces, OtlpSignal::Logs],
        batch_seconds: 5,
    }
}

#[test]
fn a_build_without_the_exporter_refuses_a_configured_collector() {
    let refused = ExportCapability::NotLinked
        .admit(Some(&collector()))
        .expect_err("a collector the build cannot export to is refused");
    assert_eq!(
        refused,
        OtlpExportUnavailable {
            endpoint: "https://collector.example:4318".to_owned()
        }
    );
    let message = refused.to_string();
    assert!(message.contains("observability.otlp"), "{message}");
    assert!(message.contains("`otel` feature"), "{message}");
}

#[test]
fn a_build_without_the_exporter_admits_a_profile_without_a_collector() {
    assert_eq!(ExportCapability::NotLinked.admit(None), Ok(()));
}

#[test]
fn a_build_with_the_exporter_admits_a_configured_collector() {
    assert_eq!(ExportCapability::COMPILED, ExportCapability::Linked);
    assert_eq!(ExportCapability::Linked.admit(Some(&collector())), Ok(()));
}
