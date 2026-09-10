//! `services publish` — push a bundle archive to an OCI registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Serialize;
use systemprompt_loader::bundle::source::push_bundle;
use systemprompt_loader::bundle::verify;

use super::signing::ENV_PREFIX;
use crate::shared::CommandOutput;

const OCI_SCHEME: &str = "oci://";

#[derive(Debug, Clone, Args)]
pub struct PublishArgs {
    #[arg(long, help = "Bundle archive to publish")]
    pub bundle: PathBuf,

    #[arg(long, help = "Destination, e.g. oci://ghcr.io/org/services:1.4.0")]
    pub to: String,

    #[arg(long, help = "Registry credential read from the profile's secrets")]
    pub auth_secret: Option<String>,

    #[arg(long, help = "Registry credential given inline as env:VAR")]
    pub auth: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublishOutcome {
    pub reference: String,
    pub digest: String,
    pub pin: String,
    pub version: String,
}

pub async fn execute(args: &PublishArgs) -> Result<CommandOutput> {
    let reference = args
        .to
        .strip_prefix(OCI_SCHEME)
        .unwrap_or(args.to.as_str())
        .to_owned();
    let signed = verify::read_manifest(&args.bundle)
        .with_context(|| format!("Failed to read {}", args.bundle.display()))?;
    let manifest_json =
        serde_json::to_vec(&signed).context("Bundle manifest is not serialisable")?;

    let secret = resolve_credential(args)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("Failed to build an HTTP client")?;

    let digest = push_bundle(&reference, &args.bundle, &manifest_json, secret, client)
        .await
        .with_context(|| format!("Failed to publish to {reference}"))?;

    let outcome = PublishOutcome {
        pin: pin_reference(&reference, &digest),
        reference,
        digest,
        version: signed.manifest.version,
    };
    Ok(CommandOutput::card_value("Bundle Published", &outcome))
}

fn resolve_credential(args: &PublishArgs) -> Result<Option<String>> {
    if let Some(inline) = args.auth.as_deref() {
        let Some(var) = inline.strip_prefix(ENV_PREFIX) else {
            bail!("--auth must be given as env:VAR so the credential is never a shell argument");
        };
        let value = std::env::var(var)
            .with_context(|| format!("Registry credential variable {var} is not set"))?;
        return Ok(Some(value));
    }
    let Some(name) = args.auth_secret.as_deref() else {
        return Ok(None);
    };
    let secrets = systemprompt_config::SecretsBootstrap::get()
        .context("--auth-secret needs an initialised profile")?;
    let value = secrets
        .get(name)
        .cloned()
        .with_context(|| format!("Secret '{name}' is not present in the profile's secrets"))?;
    Ok(Some(value))
}

#[must_use]
pub fn pin_reference(reference: &str, digest: &str) -> String {
    let repository = reference
        .split_once('@')
        .map_or(reference, |(head, _)| head);
    let repository = repository
        .rsplit_once(':')
        .filter(|(head, _)| head.contains('/'))
        .map_or(repository, |(head, _)| head);
    format!("{repository}@{digest}")
}
