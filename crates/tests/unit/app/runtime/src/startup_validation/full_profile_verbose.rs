//! Drives `StartupValidator::validate` end-to-end against a fully
//! bootstrapped tempdir profile in verbose (startup) mode, reaching the
//! domain-validation, MCP-manifest, and extension-validation phases that the
//! no-profile short-circuit tests never enter.
//!
//! The inventory fixture extensions (`ext_fixtures`) make the extension arms
//! deterministic in this binary: `covextbad` always rejects its config and
//! `covassets_missing` always misses its required asset, so the final report
//! carries exactly those extension errors when everything else is healthy.

use systemprompt_logging::set_startup_mode;
use systemprompt_models::Config;
use systemprompt_runtime::StartupValidator;

use crate::boot::{BootOptions, boot};

fn validated_config() -> Option<(crate::boot::BootFixture, Config)> {
    let fixture = boot(&BootOptions::default())?;
    systemprompt_config::try_init_config().expect("init config from profile");
    Some((fixture, Config::get().expect("config installed").clone()))
}

#[test]
fn verbose_validate_reports_only_fixture_extension_failures() {
    let Some((_fixture, config)) = validated_config() else {
        return;
    };
    set_startup_mode(true);

    std::fs::write(
        std::path::Path::new(&config.services_path).join("config/covextok.yaml"),
        "mode: fine\n",
    )
    .expect("write covextok config");

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    assert!(
        report.profile_path.is_some(),
        "profile path must be recorded on the report"
    );
    for domain in &report.domains {
        assert!(
            !domain.has_errors(),
            "domain {} unexpectedly errored: {:?}",
            domain.domain,
            domain.errors
        );
    }

    let bad = report
        .extensions
        .iter()
        .find(|e| e.domain == "ext:covextbad")
        .expect("covextbad must be reported");
    assert_eq!(bad.errors.len(), 1);
    assert_eq!(bad.errors[0].field, "covextbad.config");
    assert!(
        bad.errors[0].message.contains("fixture always rejects"),
        "got: {}",
        bad.errors[0].message
    );

    let assets = report
        .extensions
        .iter()
        .find(|e| e.domain == "ext:covassets_missing")
        .expect("covassets_missing must be reported");
    assert_eq!(assets.errors[0].field, "required_asset");
    assert!(
        assets.errors[0]
            .message
            .contains("covassets_missing/absent.css"),
        "got: {}",
        assets.errors[0].message
    );
    assert_eq!(
        assets.errors[0].suggestion.as_deref(),
        Some("Ensure the asset file exists at the specified path")
    );

    assert!(
        !report
            .extensions
            .iter()
            .any(|e| e.domain == "ext:covassets_ok"),
        "the extension whose asset exists must not be reported"
    );
    assert!(
        !report.extensions.iter().any(|e| e.domain == "ext:covextok"),
        "the extension whose config validates must not be reported"
    );
}

#[test]
fn malformed_extension_config_reports_load_failure() {
    let Some((_fixture, config)) = validated_config() else {
        return;
    };

    std::fs::write(
        std::path::Path::new(&config.services_path).join("config/covextok.yaml"),
        ": : not yaml [",
    )
    .expect("write malformed covextok config");

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    let ok_ext = report
        .extensions
        .iter()
        .find(|e| e.domain == "ext:covextok")
        .expect("covextok must fail on a malformed config file");
    assert_eq!(ok_ext.errors[0].field, "covextok.config");
    assert!(
        ok_ext.errors[0]
            .message
            .starts_with("Failed to load config:"),
        "got: {}",
        ok_ext.errors[0].message
    );
    assert!(
        ok_ext.errors[0].message.contains("covextok.yaml"),
        "got: {}",
        ok_ext.errors[0].message
    );
}

#[test]
fn malformed_ancillary_configs_degrade_to_warnings_not_errors() {
    let Some((_fixture, config)) = validated_config() else {
        return;
    };
    set_startup_mode(true);

    // Point the three optional config paths at malformed YAML files; the
    // loaders must warn and continue rather than fail validation.
    let mut config = config;
    let bad = std::path::Path::new(&config.services_path).join("bad.yaml");
    std::fs::write(&bad, ": : not yaml [").expect("write malformed yaml");
    let bad = bad.display().to_string();
    config.content_config_path.clone_from(&bad);
    config.web_config_path.clone_from(&bad);
    config.web_metadata_path.clone_from(&bad);

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    for domain in ["content", "web"] {
        let d = report
            .domains
            .iter()
            .find(|d| d.domain == domain)
            .unwrap_or_else(|| panic!("{domain} domain must still be validated"));
        assert!(
            !d.has_errors(),
            "{domain} must have no errors when its config fails to load: {:?}",
            d.errors
        );
    }
}

#[test]
fn restrictive_rate_limits_surface_as_domain_warnings() {
    let Some(fixture) = boot(&BootOptions {
        stream_per_second: 5,
        ..BootOptions::default()
    }) else {
        return;
    };
    systemprompt_config::try_init_config().expect("init config from profile");
    let _fixture = fixture;
    let config = Config::get().expect("config installed").clone();
    set_startup_mode(true);

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    let rate = report
        .domains
        .iter()
        .find(|d| d.domain == "rate_limits")
        .expect("rate_limits domain must be present");
    assert!(!rate.has_errors(), "got errors: {:?}", rate.errors);
    assert!(
        rate.warnings
            .iter()
            .any(|w| w.field == "rate_limits.stream_per_second"),
        "expected stream_per_second warning, got: {:?}",
        rate.warnings
    );
}
