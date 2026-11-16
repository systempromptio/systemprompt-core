//! Drives `StartupValidator::validate` past the services-config early-bail
//! using the process-wide bootstrap fixture, so the domain-validator,
//! MCP-manifest, and extension passes actually execute. The sibling
//! `validator.rs` covers only the uninitialised-profile bail; this module
//! covers the initialised happy path.

use std::sync::Mutex;

use serde::Deserialize;
use systemprompt_extension::{Admitted, ExtensionConfig, ExtensionConfigError};
use systemprompt_logging::OutputState;
use systemprompt_runtime::{StartupValidator, validate_extension_configs};
use systemprompt_test_fixtures::{ComposedParts, Probe, ensure_test_bootstrap, probe_admitted};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ProbeConfig {}

impl ExtensionConfig for ProbeConfig {
    const PREFIX: &'static str = "validatorprobe";

    fn validate(&self) -> Result<(), ExtensionConfigError> {
        Ok(())
    }
}

fn admitted(with_probe: bool) -> Admitted {
    let probes = if with_probe {
        vec![Probe::new("validatorprobe", |c| {
            c.config::<ProbeConfig>();
        })]
    } else {
        Vec::new()
    };
    probe_admitted(ComposedParts {
        probes,
        ..ComposedParts::default()
    })
}

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn validate_runs_full_pipeline_when_bootstrapped_quiet() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    let output = OutputState::new();
    output.set_startup_mode(false);
    let _scope = tracing::subscriber::set_default(tracing_subscriber::registry().with(output));

    let env = ensure_test_bootstrap();
    let cfg = env.boot_config().as_ref().clone();
    let mut validator = StartupValidator::new(env.boot.clone()).expect("startup validator");
    let report = validator.validate(&cfg, admitted(false).built());

    assert!(
        report.profile_path.is_some(),
        "bootstrapped run must record the profile path: {report:?}",
    );
    let domains: Vec<&str> = report.domains.iter().map(|d| d.domain.as_str()).collect();
    for expected in ["files", "web", "content", "agents", "mcp", "ai"] {
        assert!(
            domains.contains(&expected),
            "domain-validation pass must have run `{expected}`: {domains:?}",
        );
    }
}

#[test]
fn validate_runs_full_pipeline_when_bootstrapped_verbose() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    let env = ensure_test_bootstrap();

    let cfg = env.boot_config().as_ref().clone();
    let mut validator = StartupValidator::new(env.boot.clone()).expect("startup validator");
    let report = validator.validate(&cfg, admitted(false).built());

    assert!(
        report.profile_path.is_some(),
        "verbose bootstrapped run must record the profile path: {report:?}",
    );
    let domains: Vec<&str> = report.domains.iter().map(|d| d.domain.as_str()).collect();
    assert!(
        domains.contains(&"mcp"),
        "verbose pass must reach the mcp domain validator: {domains:?}",
    );
}

#[test]
fn validate_extension_configs_reports_each_admitted_config_section() {
    let env = ensure_test_bootstrap();

    let outcomes = validate_extension_configs(admitted(true).built(), &env.services_path);

    let probe = outcomes
        .iter()
        .find(|outcome| outcome.extension_id.as_str() == "validatorprobe")
        .unwrap_or_else(|| panic!("the admitted probe's config is validated: {outcomes:?}"));
    assert_eq!(probe.config_key, "validatorprobe.config");
    assert_eq!(probe.error, None);
}

#[test]
fn validate_extension_configs_reports_nothing_for_an_unadmitted_member() {
    let env = ensure_test_bootstrap();

    let outcomes = validate_extension_configs(admitted(false).built(), &env.services_path);

    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.extension_id.as_str() != "validatorprobe"),
        "a member that is not admitted is not validated: {outcomes:?}"
    );
}
