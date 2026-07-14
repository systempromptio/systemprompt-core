//! Failure arms of the profile-backed validation pipeline: an enabled
//! internal MCP server whose extension manifest cannot be resolved, and a
//! skill entry missing its content file, printed through the verbose
//! domain-error branch.

use systemprompt_models::Config;
use systemprompt_runtime::StartupValidator;

use crate::boot::{BootOptions, boot};

#[test]
fn missing_internal_mcp_manifest_stops_validation_with_mcp_error() {
    let mcp = "mcp_servers:\n  ghost:\n    type: internal\n    binary: ghost_binary\n    \
               package: ghost\n    port: 5055\n    enabled: true\n    display_in_web: false\n    \
               oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      \
               client_id: null\n";
    let Some(_fixture) = boot(&BootOptions {
        mcp_servers_yaml: mcp.to_owned(),
        ..BootOptions::default()
    }) else {
        return;
    };
    systemprompt_config::try_init_config().expect("init config from profile");
    let config = Config::get().expect("config installed").clone();

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    let mcp_domain = report
        .domains
        .iter()
        .find(|d| d.domain == "mcp")
        .expect("mcp domain must carry the manifest error");
    let err = mcp_domain
        .errors
        .iter()
        .find(|e| e.field == "mcp_servers.ghost.binary")
        .expect("manifest error keyed by the deployment binary");
    assert!(
        err.message
            .contains("Manifest not found for binary 'ghost_binary'"),
        "got: {}",
        err.message
    );
    assert_eq!(
        err.suggestion.as_deref(),
        Some("Ensure manifest.yaml exists at extensions/mcp/ghost_binary/manifest.yaml")
    );

    // Manifest errors gate extension validation entirely, so the fixture
    // extensions that otherwise always report must be absent.
    assert!(
        report.extensions.is_empty(),
        "extension validation must not run after MCP manifest errors: {:?}",
        report
            .extensions
            .iter()
            .map(|e| &e.domain)
            .collect::<Vec<_>>()
    );
}

#[test]
fn skill_missing_content_file_errors_in_verbose_mode() {
    let Some(_fixture) = boot(&BootOptions::default()) else {
        return;
    };
    systemprompt_config::try_init_config().expect("init config from profile");
    let config = Config::get().expect("config installed").clone();
    systemprompt_logging::set_startup_mode(true);

    std::fs::remove_file(
        std::path::Path::new(&config.services_path).join("skills/echo_skill/index.md"),
    )
    .expect("remove skill content file");

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    let skills = report
        .domains
        .iter()
        .find(|d| d.domain == "skills")
        .expect("skills domain must be present");
    let err = skills
        .errors
        .iter()
        .find(|e| e.field == "skills.echo_skill.file")
        .expect("missing content file must be reported");
    assert!(
        err.message.contains("'index.md' not found"),
        "got: {}",
        err.message
    );
}

#[test]
fn quiet_mode_validate_reports_the_same_extension_failures() {
    let Some(_fixture) = boot(&BootOptions::default()) else {
        return;
    };
    systemprompt_config::try_init_config().expect("init config from profile");
    let config = Config::get().expect("config installed").clone();

    let mut config = config;
    let bad = std::path::Path::new(&config.services_path).join("bad.yaml");
    std::fs::write(&bad, ": : not yaml [").expect("write malformed yaml");
    let bad = bad.display().to_string();
    config.content_config_path.clone_from(&bad);
    config.web_config_path.clone_from(&bad);
    config.web_metadata_path.clone_from(&bad);

    let mut validator = StartupValidator::new();
    let report = validator.validate(&config);

    assert!(
        report
            .extensions
            .iter()
            .any(|e| e.domain == "ext:covextbad"),
        "quiet mode must reach extension validation; got {:?}",
        report
            .extensions
            .iter()
            .map(|e| &e.domain)
            .collect::<Vec<_>>()
    );
    for domain in &report.domains {
        assert!(
            !domain.has_errors(),
            "domain {} unexpectedly errored: {:?}",
            domain.domain,
            domain.errors
        );
    }
}
