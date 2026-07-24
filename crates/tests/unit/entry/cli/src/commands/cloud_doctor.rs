//! Tests for `commands::cloud::doctor::checks` — the dependency-free
//! pre-deploy checks over the local secrets map.

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::{CheckStatus, check_proxy_topology, check_required_secrets};
use systemprompt_cloud::profile_authoring::CloudProfileBuilder;
use systemprompt_models::ProfileType;

fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn required_secrets_present_passes() {
    let secrets = map(&[
        ("oauth_at_rest_pepper", "x"),
        ("database_url", "postgres://"),
    ]);
    assert_eq!(check_required_secrets(&secrets).status, CheckStatus::Pass);
}

#[test]
fn internal_database_url_satisfies_db_requirement() {
    let secrets = map(&[
        ("oauth_at_rest_pepper", "x"),
        ("internal_database_url", "postgres://"),
    ]);
    assert_eq!(check_required_secrets(&secrets).status, CheckStatus::Pass);
}

#[test]
fn missing_pepper_fails() {
    let secrets = map(&[("database_url", "postgres://")]);
    let result = check_required_secrets(&secrets);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("oauth_at_rest_pepper"));
}

#[test]
fn missing_database_url_fails() {
    let secrets = map(&[("oauth_at_rest_pepper", "x")]);
    let result = check_required_secrets(&secrets);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("database_url"));
}

#[test]
fn proxy_topology_passes_for_generated_cloud_profile() {
    let profile = CloudProfileBuilder::new("prod").build();
    assert_eq!(check_proxy_topology(&profile).status, CheckStatus::Pass);
}

#[test]
fn proxy_topology_fails_when_fly_range_missing() {
    let mut profile = CloudProfileBuilder::new("prod").build();
    profile.server.trusted_proxies = vec!["104.16.0.0/13".parse().unwrap()];
    let result = check_proxy_topology(&profile);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("fc00::/7"));
    assert!(result.detail.contains("trusted_proxies"));
}

#[test]
fn proxy_topology_warns_when_only_fly_public_edge_missing() {
    let mut profile = CloudProfileBuilder::new("prod").build();
    profile.server.trusted_proxies = vec!["fc00::/7".parse().unwrap()];
    let result = check_proxy_topology(&profile);
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(result.detail.contains("66.241.64.0/18"));
}

#[test]
fn proxy_topology_fails_for_empty_cloud_list() {
    let mut profile = CloudProfileBuilder::new("prod").build();
    profile.server.trusted_proxies = Vec::new();
    assert_eq!(check_proxy_topology(&profile).status, CheckStatus::Fail);
}

#[test]
fn proxy_topology_skips_non_cloud_profiles() {
    let mut profile = CloudProfileBuilder::new("dev").build();
    profile.target = ProfileType::Local;
    profile.server.trusted_proxies = Vec::new();
    assert_eq!(check_proxy_topology(&profile).status, CheckStatus::Pass);
}

#[test]
fn empty_secrets_reports_both_missing() {
    let result = check_required_secrets(&HashMap::new());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("oauth_at_rest_pepper"));
    assert!(result.detail.contains("database_url"));
}
