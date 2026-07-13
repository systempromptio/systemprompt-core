//! Failure arm of the profile-backed validation pipeline: an enabled
//! internal MCP server whose extension manifest cannot be resolved.

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
