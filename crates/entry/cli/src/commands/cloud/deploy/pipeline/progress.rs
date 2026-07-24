//! Progress seam between the deploy pipeline and its presentation layer.
//!
//! The orchestrator emits a [`DeployEvent`] at every step boundary;
//! implementations decide how (or whether) to render. Events carry borrowed
//! data, so no rendering state lives in the pipeline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

pub trait DeployProgress: Send + Sync {
    fn event(&self, event: &DeployEvent<'_>);
}

#[derive(Debug, Clone, Copy)]
pub enum DeployEvent<'a> {
    ArtifactsResolved {
        tenant_name: &'a str,
        binary: &'a Path,
        dockerfile: &'a Path,
    },
    RegistryAuthStarted,
    RegistryAuthFinished,
    ImageResolved {
        image: &'a str,
    },
    BuildStarted,
    BuildFinished,
    PushSkipped,
    PushStarted,
    PushFinished,
    SecretsPhaseStarted,
    SecretsFileMissing,
    SecretsSyncStarted,
    SecretsSynced {
        count: usize,
    },
    CredentialsSyncStarted,
    CredentialsSynced {
        count: usize,
    },
    ProfilePathConfigured,
    DeployStarted,
    Deployed {
        status: &'a str,
        app_url: Option<&'a str>,
    },
}
