//! `services inspect` — bundle provenance for an archive or the active
//! composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Serialize;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::ServicesRootBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root, verify};
use systemprompt_models::Profile;
use systemprompt_models::services::bundle::SignedBundleManifest;
use systemprompt_security::manifest_signing::{canonical_manifest_bytes, verify_with_pubkey};

use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct InspectArgs {
    #[arg(long, help = "Bundle archive to inspect")]
    pub bundle: Option<PathBuf>,

    #[arg(
        long,
        conflicts_with = "bundle",
        help = "Inspect the composition this instance is running"
    )]
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct BundleReport {
    pub version: String,
    pub created_at: String,
    pub requires_core: String,
    pub content_hash: String,
    pub source_repo: Option<String>,
    pub source_commit: Option<String>,
    pub workflow_run: Option<String>,
    pub files: usize,
    pub total_size: u64,
    pub owns: String,
    pub signature_key_id: Option<String>,
    pub signature_status: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveReport {
    pub path: String,
    pub provenance: String,
    pub composed_hash: Option<String>,
    pub last_reconciled_hash: Option<String>,
    pub sources: String,
    pub detail: Option<String>,
}

pub fn execute(args: &InspectArgs) -> Result<CommandOutput> {
    if let Some(archive) = args.bundle.as_deref() {
        let signed = verify::read_manifest(archive)
            .with_context(|| format!("Failed to read {}", archive.display()))?;
        let report = describe_bundle(&signed, ProfileBootstrap::get().ok());
        return Ok(CommandOutput::card_value("Services Bundle", &report));
    }
    if !args.active {
        bail!("Pass --bundle <archive> or --active");
    }
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(CommandOutput::card_value(
        "Active Services Root",
        &describe_active(profile)?,
    ))
}

#[must_use]
pub fn describe_bundle(signed: &SignedBundleManifest, profile: Option<&Profile>) -> BundleReport {
    let manifest = &signed.manifest;
    BundleReport {
        version: manifest.version.clone(),
        created_at: manifest.created_at.to_rfc3339(),
        requires_core: manifest.requires_core.clone(),
        content_hash: manifest.content_hash.clone(),
        source_repo: manifest.source.repo.clone(),
        source_commit: manifest.source.commit.clone(),
        workflow_run: manifest.source.workflow_run.clone(),
        files: manifest.files.len(),
        total_size: manifest.total_size,
        owns: owns_summary(manifest),
        signature_key_id: signed.signature.as_ref().map(|s| s.key_id.clone()),
        signature_status: signature_status(signed, profile),
    }
}

fn owns_summary(
    manifest: &systemprompt_models::services::bundle::ServicesBundleManifest,
) -> String {
    let owns = &manifest.owns;
    format!(
        "{} marketplace(s), {} plugin(s), {} skill(s), {} rule(s), {} hook(s), {} artifact(s); \
         dirs: {}",
        owns.marketplaces.len(),
        owns.plugins.len(),
        owns.skills.len(),
        owns.rules.len(),
        owns.hooks.len(),
        owns.artifacts.len(),
        owns.dirs.join(", ")
    )
}

fn signature_status(signed: &SignedBundleManifest, profile: Option<&Profile>) -> String {
    let Some(signature) = signed.signature.as_ref() else {
        return "unsigned".to_owned();
    };
    let Some(profile) = profile else {
        return "signed (no profile to check pinned keys against)".to_owned();
    };
    let pinned: Vec<&String> = profile
        .services
        .sources
        .iter()
        .filter_map(|source| source.verification())
        .flat_map(|verification| verification.ed25519_public_keys.iter())
        .collect();
    if pinned.is_empty() {
        return "signed (profile pins no keys)".to_owned();
    }
    let Ok(payload) = canonical_manifest_bytes(&signed.manifest) else {
        return "signed (manifest could not be canonicalised)".to_owned();
    };
    let verified = pinned
        .iter()
        .any(|key| verify_with_pubkey(key, &payload, &signature.sig_b64).is_ok());
    if verified {
        "verified against a pinned key".to_owned()
    } else {
        "signed by a key this profile does not pin".to_owned()
    }
}

fn describe_active(profile: &Profile) -> Result<ActiveReport> {
    let root = ServicesRootBootstrap::get()
        .context("The services root has not been resolved in this process")?;
    let state = BundleCache::new(cache_root(profile)).read_state();
    let sources = state
        .sources
        .iter()
        .map(|(name, s)| format!("{name}={} ({})", s.version, s.digest))
        .collect::<Vec<_>>()
        .join(", ");

    let (provenance, composed_hash, detail) = match &root.provenance {
        systemprompt_loader::ServicesProvenance::Bundled => ("bundled", None, None),
        systemprompt_loader::ServicesProvenance::Fetched { composed_hash, .. } => {
            ("fetched", Some(composed_hash.clone()), None)
        },
        systemprompt_loader::ServicesProvenance::LastGood {
            composed_hash,
            error,
        } => (
            "last_good",
            Some(composed_hash.clone()),
            Some(error.clone()),
        ),
        systemprompt_loader::ServicesProvenance::BundledFallback { error } => {
            ("bundled_fallback", None, Some(error.clone()))
        },
    };

    Ok(ActiveReport {
        path: root.path.display().to_string(),
        provenance: provenance.to_owned(),
        composed_hash,
        last_reconciled_hash: state.last_reconciled_hash,
        sources,
        detail,
    })
}
