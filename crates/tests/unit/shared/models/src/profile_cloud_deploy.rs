use serde_json::{Value, json};
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::CloudConfig;

use crate::profile_services_sources::local_profile;

fn deploy() -> Value {
    json!({
        "registry": "europe-docker.pkg.dev",
        "repository": "proj/tenant",
        "gitops": {
            "repo_path": "/srv/ops",
            "values_file": "tenants/a/values.yaml",
            "image_digest_key": "image.digest",
            "branch": "main",
        },
    })
}

fn operator() -> Value {
    json!({ "issuer": "https://idp.example.com", "client_id": "cli" })
}

fn cloud_errors(cloud: Value) -> Vec<String> {
    let mut profile: Profile = local_profile();
    profile.cloud = Some(serde_json::from_value::<CloudConfig>(cloud).unwrap());
    match profile.validate() {
        Ok(()) => Vec::new(),
        Err(systemprompt_manifest::profile::ProfileError::Validation { errors, .. }) => errors
            .into_iter()
            .filter(|e| e.contains("cloud.deploy") || e.contains("cloud.operator"))
            .collect(),
        Err(other) => panic!("unexpected error: {other}"),
    }
}

fn with_deploy(edit: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut deploy = deploy();
    edit(&mut deploy);
    cloud_errors(json!({ "provider": "gke", "deploy": deploy }))
}

#[test]
fn a_well_formed_gke_deploy_and_operator_pass() {
    let errors = cloud_errors(json!({
        "provider": "gke",
        "deploy": deploy(),
        "operator": operator(),
    }));
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn deploy_on_a_managed_provider_is_refused() {
    let errors = cloud_errors(json!({ "provider": "systemprompt", "deploy": deploy() }));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("provider 'systemprompt'"), "{errors:?}");
}

#[test]
fn blank_deploy_fields_are_each_named() {
    let errors = with_deploy(|d| {
        d["registry"] = json!(" ");
        d["repository"] = json!("");
        d["gitops"]["branch"] = json!("");
        d["gitops"]["commit_message"] = json!(" ");
    });
    for field in [
        "cloud.deploy.registry must not be empty",
        "cloud.deploy.repository must not be empty",
        "cloud.deploy.gitops.branch must not be empty",
        "cloud.deploy.gitops.commit_message must not be empty",
    ] {
        assert!(errors.iter().any(|e| e == field), "{field}: {errors:?}");
    }
}

#[test]
fn a_registry_url_and_a_tagged_repository_are_refused() {
    let errors = with_deploy(|d| {
        d["registry"] = json!("https://europe-docker.pkg.dev");
        d["repository"] = json!("proj/tenant:latest");
    });
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains("not a URL"));
    assert!(errors[1].contains("without a tag or digest"));
}

#[test]
fn a_digest_pinned_repository_is_refused() {
    let errors = with_deploy(|d| d["repository"] = json!("proj/tenant@sha256:abc"));
    assert_eq!(errors.len(), 1, "{errors:?}");
}

#[test]
fn empty_gitops_paths_are_refused() {
    let errors = with_deploy(|d| {
        d["gitops"]["repo_path"] = json!("");
        d["gitops"]["values_file"] = json!("");
    });
    assert!(errors.contains(&"cloud.deploy.gitops.repo_path must not be empty".to_owned()));
    assert!(errors.contains(&"cloud.deploy.gitops.values_file must not be empty".to_owned()));
}

#[test]
fn a_values_file_outside_the_repo_is_refused() {
    for escape in ["/etc/values.yaml", "../other/values.yaml"] {
        let errors = with_deploy(|d| d["gitops"]["values_file"] = json!(escape));
        assert_eq!(errors.len(), 1, "{escape}: {errors:?}");
        assert!(errors[0].contains(escape), "{errors:?}");
    }
}

#[test]
fn a_digest_key_with_an_empty_segment_is_refused() {
    for key in ["image..digest", ".digest", ""] {
        let errors = with_deploy(|d| d["gitops"]["image_digest_key"] = json!(key));
        assert_eq!(errors.len(), 1, "{key}: {errors:?}");
        assert!(errors[0].contains("dotted mapping path"), "{errors:?}");
    }
}

#[test]
fn operator_issuer_client_and_scopes_are_checked() {
    let errors = cloud_errors(json!({
        "provider": "kubernetes",
        "operator": {
            "issuer": "http://idp.example.com",
            "client_id": "cli",
            "scopes": ["email"],
        },
    }));
    assert_eq!(
        errors,
        vec![
            "cloud.operator.issuer must be an https URL, got: 'http://idp.example.com'".to_owned(),
            "cloud.operator.scopes must include 'openid'".to_owned(),
        ]
    );
}

#[test]
fn a_blank_operator_client_id_is_refused_when_the_profile_is_read() {
    let error = serde_json::from_value::<CloudConfig>(json!({
        "provider": "kubernetes",
        "operator": { "issuer": "https://idp.example.com", "client_id": " " },
    }))
    .expect_err("a blank client id does not parse");
    assert!(error.to_string().contains("ClientId"), "{error}");
}
