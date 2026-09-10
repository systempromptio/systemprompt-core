//! `services bundle` — pack a services tree into a signed archive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_loader::bundle::{pack, verify};
use systemprompt_models::services::bundle::{
    BUNDLE_SIGNATURE_ALG, BundleSignature, BundleSourceInfo, SignedBundleManifest,
};
use systemprompt_security::manifest_signing::{canonical_manifest_bytes, sign_with_seed};

use super::signing::{BundleSigningKey, load_signing_key};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct BundleArgs {
    #[arg(long, help = "Services tree to pack")]
    pub root: PathBuf,

    #[arg(long, help = "Archive to write (.tar.gz)")]
    pub out: PathBuf,

    #[arg(long, help = "Bundle version stamped into bundle.json")]
    pub version: String,

    #[arg(long, help = "Signing key: a file holding the base64 seed, or env:VAR")]
    pub sign_key: Option<String>,

    #[arg(long, help = "Repository the tree was authored in")]
    pub source_repo: Option<String>,

    #[arg(long, help = "Commit the tree was packed from")]
    pub source_commit: Option<String>,

    #[arg(long, help = "CI run that produced the archive")]
    pub workflow_run: Option<String>,

    #[arg(
        long,
        help = "Refuse to pack any directory outside the marketplace set"
    )]
    pub marketplace_only: bool,
}

#[derive(Debug, Serialize)]
pub struct BundleOutcome {
    pub archive: String,
    pub version: String,
    pub requires_core: String,
    pub content_hash: String,
    pub files: usize,
    pub total_size: u64,
    pub dirs: String,
    pub signed_by: Option<String>,
    pub public_key: Option<String>,
}

pub fn execute(args: &BundleArgs) -> Result<CommandOutput> {
    let key = args.sign_key.as_deref().map(load_signing_key).transpose()?;
    let outcome = pack_bundle(args, key.as_ref())?;
    Ok(CommandOutput::card_value("Services Bundle", &outcome))
}

pub fn pack_bundle(args: &BundleArgs, key: Option<&BundleSigningKey>) -> Result<BundleOutcome> {
    let manifest = pack::build_manifest(
        &args.root,
        &args.version,
        &requires_core(env!("CARGO_PKG_VERSION"))?,
        BundleSourceInfo {
            repo: args.source_repo.clone(),
            commit: args.source_commit.clone(),
            workflow_run: args.workflow_run.clone(),
        },
    )
    .with_context(|| format!("Failed to read services tree {}", args.root.display()))?;

    if args.marketplace_only {
        verify::require_marketplace_only(&manifest)
            .context("--marketplace-only refused this tree")?;
    }

    let signature = key.map(|key| sign_manifest(key, &manifest)).transpose()?;
    let signed = SignedBundleManifest {
        manifest,
        signature,
    };

    pack::write_tarball(&args.root, &signed, &args.out)
        .with_context(|| format!("Failed to write {}", args.out.display()))?;

    Ok(describe(&args.out, &signed, key))
}

fn sign_manifest(
    key: &BundleSigningKey,
    manifest: &systemprompt_models::services::bundle::ServicesBundleManifest,
) -> Result<BundleSignature> {
    let payload =
        canonical_manifest_bytes(manifest).context("Manifest could not be canonicalised")?;
    Ok(BundleSignature {
        alg: BUNDLE_SIGNATURE_ALG.to_owned(),
        key_id: key.key_id.clone(),
        sig_b64: sign_with_seed(&key.seed, &payload),
    })
}

fn describe(
    out: &Path,
    signed: &SignedBundleManifest,
    key: Option<&BundleSigningKey>,
) -> BundleOutcome {
    BundleOutcome {
        archive: out.display().to_string(),
        version: signed.manifest.version.clone(),
        requires_core: signed.manifest.requires_core.clone(),
        content_hash: signed.manifest.content_hash.clone(),
        files: signed.manifest.files.len(),
        total_size: signed.manifest.total_size,
        dirs: signed.manifest.owns.dirs.join(", "),
        signed_by: signed.signature.as_ref().map(|s| s.key_id.clone()),
        public_key: key.map(|k| k.public_key.clone()),
    }
}

pub fn requires_core(core_version: &str) -> Result<String> {
    let parsed = semver::Version::parse(core_version)
        .with_context(|| format!("Core version {core_version} is not semver"))?;
    Ok(format!(">={}.{}", parsed.major, parsed.minor))
}
