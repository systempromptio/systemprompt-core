//! Unit tests for the pure MCP manifest-validation seam extracted from
//! `validate_mcp_manifests`: `collect_manifest_errors` (the enabled /
//! `dev_only` / server-type filter matrix and the resolver error branch) and
//! `merge_mcp_errors` (append-to-existing vs. create-fresh `mcp` domain).

use std::collections::HashMap;

use systemprompt_models::ServicesConfig;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::{Deployment, McpServerType, OAuthRequirement};
use systemprompt_runtime::{collect_manifest_errors, merge_mcp_errors};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

fn deployment(
    server_type: McpServerType,
    enabled: bool,
    dev_only: bool,
    binary: &str,
) -> Deployment {
    Deployment {
        server_type,
        binary: binary.to_owned(),
        package: None,
        port: 5100,
        endpoint: None,
        enabled,
        display_in_web: false,
        dev_only,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::new(),
    }
}

fn services_with(servers: Vec<(&str, Deployment)>) -> ServicesConfig {
    let mut cfg = ServicesConfig::default();
    for (name, dep) in servers {
        cfg.mcp_servers.insert(name.to_owned(), dep);
    }
    cfg
}

#[test]
fn internal_missing_manifest_yields_error() {
    let cfg = services_with(vec![(
        "srv_a",
        deployment(McpServerType::Internal, true, false, "bin_a"),
    )]);

    let errors = collect_manifest_errors(&cfg, false, |_| Err("not found".to_owned()));

    assert_eq!(errors.len(), 1, "one internal server missing its manifest");
    assert_eq!(errors[0].field, "mcp_servers.srv_a.binary");
    assert!(
        errors[0].message.contains("bin_a"),
        "message names the binary: {}",
        errors[0].message
    );
    assert!(
        errors[0].suggestion.is_some(),
        "carries a manifest-path hint"
    );
}

#[test]
fn internal_present_manifest_yields_no_error() {
    let cfg = services_with(vec![(
        "srv_ok",
        deployment(McpServerType::Internal, true, false, "bin_ok"),
    )]);

    let errors = collect_manifest_errors(&cfg, false, |_| Ok(()));

    assert!(errors.is_empty(), "resolvable manifest produces no error");
}

#[test]
fn disabled_external_and_cloud_devonly_are_skipped() {
    let cfg = services_with(vec![
        (
            "disabled",
            deployment(McpServerType::Internal, false, false, "bin_d"),
        ),
        (
            "external",
            deployment(McpServerType::External, true, false, ""),
        ),
        (
            "dev",
            deployment(McpServerType::Internal, true, true, "bin_dev"),
        ),
    ]);

    let cloud_errors = collect_manifest_errors(&cfg, true, |_| Err("nope".to_owned()));
    assert!(
        cloud_errors.is_empty(),
        "disabled + external + cloud/dev_only servers never resolve: {cloud_errors:?}"
    );

    let local_errors = collect_manifest_errors(&cfg, false, |_| Err("nope".to_owned()));
    assert_eq!(
        local_errors.len(),
        1,
        "off-cloud, only the dev_only internal server is checked and fails: {local_errors:?}"
    );
    assert_eq!(local_errors[0].field, "mcp_servers.dev.binary");
}

#[test]
fn merge_into_empty_report_creates_mcp_domain() {
    let mut report = StartupValidationReport::new();
    merge_mcp_errors(
        &mut report,
        vec![ValidationError::new("mcp_servers.x.binary", "missing")],
    );

    let mcp = report
        .domains
        .iter()
        .find(|d| d.domain == "mcp")
        .expect("mcp domain created");
    assert_eq!(mcp.errors.len(), 1);
}

#[test]
fn merge_appends_to_existing_mcp_domain() {
    let mut report = StartupValidationReport::new();
    let mut existing = ValidationReport::new("mcp");
    existing.add_error(ValidationError::new("mcp.pre", "pre-existing"));
    report.add_domain(existing);

    merge_mcp_errors(
        &mut report,
        vec![ValidationError::new("mcp_servers.y.binary", "missing")],
    );

    let mcp_domains: Vec<_> = report
        .domains
        .iter()
        .filter(|d| d.domain == "mcp")
        .collect();
    assert_eq!(mcp_domains.len(), 1, "must not create a second mcp domain");
    assert_eq!(
        mcp_domains[0].errors.len(),
        2,
        "appended to the existing domain"
    );
}

#[test]
fn merge_empty_errors_is_noop() {
    let mut report = StartupValidationReport::new();
    merge_mcp_errors(&mut report, vec![]);
    assert!(report.domains.is_empty(), "empty error set adds no domain");
}
