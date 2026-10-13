//! Profile validation and the profile-only `cloud doctor` checks must agree:
//! a profile validation rejects has a failing check, and a profile it accepts
//! has none. 0.63.0 shipped with the two disagreeing about
//! `server.instance_id`, which made every cloud profile undeployable.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::doctor::{CheckStatus, profile_checks};
use systemprompt_cloud::profile_authoring::CloudProfileBuilder;
use systemprompt_identifiers::{InstanceId, TenantId};
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::TrustedIssuer;

fn cloud_profile() -> Profile {
    let api_url = "https://api.systemprompt.io";
    CloudProfileBuilder::new("prod")
        .with_tenant_id(TenantId::new("tenant_1"))
        .with_external_db_access(false)
        .with_secrets_path("./secrets.json")
        .with_geoip_database(None)
        .with_external_url("https://tenant-1.systemprompt.io")
        .with_trusted_issuer(TrustedIssuer {
            issuer: api_url.to_owned(),
            jwks_uri: format!("{api_url}/.well-known/jwks.json"),
            audience: "tenant_1".to_owned(),
            typ_allowlist: Vec::new(),
            allowed_client_ids: Vec::new(),
            can_issue_id_jag: false,
        })
        .build()
}

fn local_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).unwrap();
    serde_yaml::from_str(&yaml).unwrap()
}

fn failing_checks(profile: &Profile) -> Vec<String> {
    profile_checks(profile)
        .into_iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .map(|c| format!("{}: {}", c.name, c.detail))
        .collect()
}

fn assert_agree(label: &str, profile: &Profile) -> bool {
    let validation = profile.validate();
    let failing = failing_checks(profile);
    assert_eq!(
        validation.is_err(),
        !failing.is_empty(),
        "{label}: validation = {validation:?}, failing doctor checks = {failing:?}"
    );
    validation.is_err()
}

#[test]
fn a_generated_cloud_profile_validates_and_passes_the_doctor() {
    let profile = cloud_profile();
    profile
        .validate()
        .expect("generated cloud profile validates");
    assert!(
        failing_checks(&profile).is_empty(),
        "{:?}",
        failing_checks(&profile)
    );
}

#[test]
fn the_local_fixture_profile_validates_and_passes_the_doctor() {
    let profile = local_profile();
    profile.validate().expect("local fixture profile validates");
    assert!(
        failing_checks(&profile).is_empty(),
        "{:?}",
        failing_checks(&profile)
    );
}

#[test]
fn validation_and_doctor_agree_on_every_cloud_governed_field() {
    let mutations: Vec<(&str, fn(&mut Profile))> = vec![
        ("instance_id set", |p| {
            p.server.instance_id = Some(InstanceId::new("node-a"));
        }),
        ("trusted_proxies empty", |p| {
            p.server.trusted_proxies.clear()
        }),
        ("loopback api_external_url", |p| {
            p.server.api_external_url = "http://127.0.0.1:8080".to_owned();
        }),
        ("governance authz missing", |p| {
            if let Some(governance) = p.governance.as_mut() {
                governance.authz = None;
            }
        }),
    ];
    for (label, mutate) in mutations {
        let mut profile = cloud_profile();
        mutate(&mut profile);
        assert!(
            assert_agree(label, &profile),
            "{label}: expected validation to reject the cloud profile"
        );
    }
}

#[test]
fn a_cloud_profile_without_the_fly_peer_range_validates_and_only_warns() {
    let mut profile = cloud_profile();
    profile.server.trusted_proxies = vec!["10.0.0.0/8".parse().unwrap()];
    assert!(!assert_agree(
        "cloud trusted_proxies without fc00::/7",
        &profile
    ));
}

#[test]
fn an_empty_trusted_proxies_list_does_not_fail_a_local_profile() {
    let mut profile = local_profile();
    profile.server.trusted_proxies.clear();
    assert!(!assert_agree("local empty trusted_proxies", &profile));
}
