//! `cloud doctor` Git credential check: a credential under its bare label
//! fails and names the migrate command; a host-bound one passes.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::CheckStatus;
use systemprompt_cli::cloud::doctor::git_credential_checks::classify_git_credentials;
use systemprompt_identifiers::ManagedSourceId;
use systemprompt_marketplace::managed::GitCredentialSource;

fn source(label: &str) -> GitCredentialSource {
    GitCredentialSource {
        id: ManagedSourceId::new("src-1"),
        name: "skills".to_owned(),
        repository: "https://github.com/org/skills.git".to_owned(),
        credential_reference: label.to_owned(),
    }
}

fn secrets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn a_bare_label_secret_fails_and_names_the_migrate_command() {
    let result = classify_git_credentials(&secrets(&[("CI", "tok")]), &[source("ci")]);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result
            .detail
            .contains("admin config secret migrate-git-credentials --apply"),
        "{}",
        result.detail
    );
    assert!(result.detail.contains("GIT_SOURCE__GITHUB_2ECOM__CI"));
    assert!(!result.detail.contains("tok"));
}

#[test]
fn a_host_bound_secret_passes() {
    let result = classify_git_credentials(
        &secrets(&[("GIT_SOURCE__GITHUB_2ECOM__CI", "tok")]),
        &[source("ci")],
    );
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
}

#[test]
fn a_label_with_no_secret_warns() {
    let result = classify_git_credentials(&secrets(&[]), &[source("ci")]);
    assert_eq!(result.status, CheckStatus::Warn);
}
