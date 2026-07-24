//! Unit tests for `cloud::deploy::progress`.
//!
//! [`spinner_message`] is a pure event→label mapping. [`CliDeployProgress`] is
//! driven over every [`DeployEvent`] variant to prove the render path never
//! panics.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::cloud::deploy::pipeline::{DeployEvent, DeployProgress};
use systemprompt_cli::cloud::deploy::progress::{CliDeployProgress, spinner_message};

#[test]
fn spinner_message_maps_long_running_starts() {
    assert_eq!(
        spinner_message(&DeployEvent::BuildStarted),
        Some("Building Docker image...")
    );
    assert_eq!(
        spinner_message(&DeployEvent::PushStarted),
        Some("Pushing to registry...")
    );
    assert_eq!(
        spinner_message(&DeployEvent::DeployStarted),
        Some("Deploying...")
    );
}

#[test]
fn spinner_message_is_none_for_terminal_events() {
    assert_eq!(spinner_message(&DeployEvent::BuildFinished), None);
    assert_eq!(spinner_message(&DeployEvent::PushFinished), None);
    assert_eq!(spinner_message(&DeployEvent::ProfilePathConfigured), None);
}

#[test]
fn event_renders_every_variant_without_panic() {
    let binary = Path::new("/tmp/bin");
    let dockerfile = Path::new("/tmp/Dockerfile");

    let events = vec![
        DeployEvent::ArtifactsResolved {
            tenant_name: "acme",
            binary,
            dockerfile,
        },
        DeployEvent::RegistryAuthStarted,
        DeployEvent::RegistryAuthFinished,
        DeployEvent::ImageResolved { image: "img:1" },
        DeployEvent::BuildStarted,
        DeployEvent::BuildFinished,
        DeployEvent::PushSkipped,
        DeployEvent::PushStarted,
        DeployEvent::PushFinished,
        DeployEvent::SecretsPhaseStarted,
        DeployEvent::SecretsFileMissing,
        DeployEvent::SecretsSyncStarted,
        DeployEvent::SecretsSynced { count: 4 },
        DeployEvent::CredentialsSyncStarted,
        DeployEvent::CredentialsSynced { count: 5 },
        DeployEvent::ProfilePathConfigured,
        DeployEvent::DeployStarted,
        DeployEvent::Deployed {
            status: "running",
            app_url: Some("https://example.com"),
        },
        DeployEvent::Deployed {
            status: "running",
            app_url: None,
        },
    ];

    let progress = CliDeployProgress::new();
    for event in &events {
        progress.event(event);
    }
}
