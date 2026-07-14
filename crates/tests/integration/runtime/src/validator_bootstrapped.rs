//! Drives `StartupValidator::validate` past the services-config early-bail
//! using the process-wide bootstrap fixture, so the domain-validator,
//! MCP-manifest, and extension passes actually execute. The sibling
//! `validator.rs` covers only the uninitialised-profile bail; this module
//! covers the initialised happy path.

use std::sync::Mutex;

use systemprompt_logging::set_startup_mode;
use systemprompt_models::Config;
use systemprompt_runtime::{StartupValidator, validate_extension_configs};
use systemprompt_test_fixtures::ensure_test_bootstrap;

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn validate_runs_full_pipeline_when_bootstrapped_quiet() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    ensure_test_bootstrap();
    set_startup_mode(false);

    let cfg = Config::get().expect("config initialised").clone();
    let mut validator = StartupValidator::new();
    let report = validator.validate(&cfg);

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
    ensure_test_bootstrap();
    set_startup_mode(true);

    let cfg = Config::get().expect("config initialised").clone();
    let mut validator = StartupValidator::default();
    let report = validator.validate(&cfg);
    set_startup_mode(false);

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
fn validate_extension_configs_over_bootstrapped_services_path() {
    let env = ensure_test_bootstrap();

    let outcomes =
        validate_extension_configs(&env.services_path).expect("extension registry discovers");

    for outcome in &outcomes {
        assert!(
            !outcome.extension_id.is_empty(),
            "each outcome names its extension: {outcome:?}",
        );
        assert!(
            outcome.config_key.ends_with(".config"),
            "config_key is the `<prefix>.config` field name: {outcome:?}",
        );
    }
}
