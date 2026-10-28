//! `cloud.deploy` and `cloud.operator` profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{DeployConfig, OperatorAuthConfig, Profile};

impl Profile {
    pub(crate) fn validate_cloud_deploy(&self, errors: &mut Vec<String>) {
        let Some(cloud) = self.cloud.as_ref() else {
            return;
        };
        if let Some(deploy) = cloud.deploy.as_ref() {
            if !cloud.provider.is_self_managed() {
                errors.push(format!(
                    "cloud.deploy applies to the kubernetes and gke providers; provider '{}' \
                     deploys without it",
                    cloud.provider
                ));
            }
            validate_deploy(deploy, errors);
        }
        if let Some(operator) = cloud.operator.as_ref() {
            validate_operator(operator, errors);
        }
    }
}

fn validate_deploy(deploy: &DeployConfig, errors: &mut Vec<String>) {
    for (field, value) in [
        ("cloud.deploy.registry", deploy.registry.as_str()),
        ("cloud.deploy.repository", deploy.repository.as_str()),
        ("cloud.deploy.gitops.branch", deploy.gitops.branch.as_str()),
        (
            "cloud.deploy.gitops.commit_message",
            deploy.gitops.commit_message.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            errors.push(format!("{field} must not be empty"));
        }
    }
    if deploy.registry.contains("://") {
        errors.push(
            "cloud.deploy.registry is a registry host (e.g. europe-docker.pkg.dev), not a URL"
                .to_owned(),
        );
    }
    if deploy.repository.contains(':') || deploy.repository.contains('@') {
        errors.push(
            "cloud.deploy.repository names the image repository without a tag or digest".to_owned(),
        );
    }
    let gitops = &deploy.gitops;
    if gitops.repo_path.as_os_str().is_empty() {
        errors.push("cloud.deploy.gitops.repo_path must not be empty".to_owned());
    }
    if gitops.values_file.as_os_str().is_empty() {
        errors.push("cloud.deploy.gitops.values_file must not be empty".to_owned());
    } else if gitops.values_file.is_absolute()
        || gitops
            .values_file
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        errors.push(format!(
            "cloud.deploy.gitops.values_file must be a path inside repo_path, got: {}",
            gitops.values_file.display()
        ));
    }
    if gitops
        .digest_key_segments()
        .iter()
        .any(|segment| segment.trim().is_empty())
    {
        errors.push(format!(
            "cloud.deploy.gitops.image_digest_key must be a dotted mapping path (e.g. \
             image.digest), got: '{}'",
            gitops.image_digest_key
        ));
    }
}

fn validate_operator(operator: &OperatorAuthConfig, errors: &mut Vec<String>) {
    if !operator.issuer.starts_with("https://") {
        errors.push(format!(
            "cloud.operator.issuer must be an https URL, got: '{}'",
            operator.issuer
        ));
    }
    if operator.client_id.trim().is_empty() {
        errors.push("cloud.operator.client_id must not be empty".to_owned());
    }
    if !operator.scopes.iter().any(|scope| scope == "openid") {
        errors.push("cloud.operator.scopes must include 'openid'".to_owned());
    }
}
