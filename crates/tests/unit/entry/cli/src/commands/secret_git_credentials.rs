//! `admin config secret migrate-git-credentials`: bare-label Git credentials
//! move to `git_source/<host>/<label>` in the secrets file, never overwriting
//! a namespaced secret and never reporting a value.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::config::git_credentials::{CredentialStatus, migrate_file};
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};
use systemprompt_identifiers::ManagedSourceId;
use systemprompt_marketplace::managed::GitCredentialSource;

fn source(name: &str, repository: &str, label: &str) -> GitCredentialSource {
    GitCredentialSource {
        id: ManagedSourceId::new(format!("src-{name}")),
        name: name.to_owned(),
        repository: repository.to_owned(),
        credential_reference: label.to_owned(),
    }
}

fn secrets_file(dir: &tempfile::TempDir, body: serde_json::Value) -> std::path::PathBuf {
    let path = dir.path().join("secrets.json");
    std::fs::write(&path, body.to_string()).unwrap();
    path
}

fn read(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn dry_run_reports_every_outcome_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let body = serde_json::json!({
        "gemini": "g",
        "bare": "tok-bare",
        "done": "tok-done",
        "git_source/github.com/done": "tok-done",
        "clash": "tok-old",
        "git_source/github.com/clash": "tok-new",
    });
    let path = secrets_file(&dir, body.clone());
    let sources = [
        source("a", "https://github.com/acme/a", "bare"),
        source("b", "https://github.com/acme/b", "done"),
        source("c", "https://github.com/acme/c", "missing"),
        source("d", "https://github.com/acme/d", "clash"),
    ];
    let out = migrate_file(&path, &sources, false).unwrap();
    let statuses: Vec<_> = out.sources.iter().map(|r| r.status).collect();
    assert_eq!(
        statuses,
        vec![
            CredentialStatus::Renamed,
            CredentialStatus::AlreadyNamespaced,
            CredentialStatus::NoSecretFound,
            CredentialStatus::Conflict,
        ]
    );
    assert_eq!(read(&path), body, "a dry run writes nothing");
    let rendered = serde_json::to_string(&out).unwrap();
    for value in ["tok-bare", "tok-done", "tok-old", "tok-new"] {
        assert!(!rendered.contains(value), "the report leaks {value}");
    }
}

#[test]
fn apply_renames_without_overwriting_and_keeps_conflicting_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = secrets_file(
        &dir,
        serde_json::json!({
            "shared": "tok-shared",
            "clash": "tok-old",
            "git_source/gitlab.com/clash": "tok-new",
        }),
    );
    let sources = [
        source("a", "https://github.com/acme/a", "shared"),
        source("b", "https://gitlab.com/acme/b", "shared"),
        source("c", "https://gitlab.com/acme/c", "clash"),
    ];
    let out = migrate_file(&path, &sources, true).unwrap();
    assert_eq!(out.removed_bare_labels, vec!["shared".to_owned()]);
    assert_eq!(
        read(&path),
        serde_json::json!({
            "git_source/github.com/shared": "tok-shared",
            "git_source/gitlab.com/shared": "tok-shared",
            "clash": "tok-old",
            "git_source/gitlab.com/clash": "tok-new",
        })
    );
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn the_migration_is_destructive() {
    let cli = Cli::try_parse_from([
        "systemprompt",
        "admin",
        "config",
        "secret",
        "migrate-git-credentials",
    ])
    .unwrap();
    let desc = cli
        .command
        .as_ref()
        .map_or(CommandDescriptor::FULL, Commands::descriptor);
    assert_eq!(desc.data_impact(), DataImpact::Destructive);
    assert!(desc.database());
}
