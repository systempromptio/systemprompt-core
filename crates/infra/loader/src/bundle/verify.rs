//! Bundle verification, in the order the trust chain requires.
//!
//! The archive digest is checked before a single byte is parsed, the manifest
//! signature before the manifest is believed, and the per-file checksums
//! before the extracted tree is used. A bundle that fails any step is never
//! installed and never cached: there is no warn-and-continue path here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use systemprompt_models::profile::BundleVerification;
use systemprompt_models::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BUNDLE_FORMAT_VERSION, BUNDLE_MANIFEST_FILE, BUNDLE_SIGNATURE_ALG,
    ServicesBundleManifest, SignedBundleManifest,
};
use systemprompt_security::manifest_signing::{
    canonical_manifest_bytes, key_id_for_pubkey, verify_with_pubkey,
};
use tar::Archive;

use super::error::{BundleError, BundleResult, VerifyFailure};
use super::pack::collect_files;

pub fn file_digest(path: &Path) -> BundleResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn verify_bundle(
    archive: &Path,
    verification: &BundleVerification,
    core_version: &str,
) -> BundleResult<SignedBundleManifest> {
    if let Some(expected) = verification.sha256.as_deref() {
        let actual = file_digest(archive)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(VerifyFailure::DigestMismatch {
                expected: expected.to_owned(),
                actual,
            }
            .into());
        }
    }

    let signed = read_manifest(archive)?;
    if signed.manifest.format != BUNDLE_FORMAT_VERSION {
        return Err(VerifyFailure::UnsupportedFormat {
            format: signed.manifest.format,
            supported: BUNDLE_FORMAT_VERSION,
        }
        .into());
    }

    verify_signature(&signed, &verification.ed25519_public_keys)?;

    let satisfied = signed
        .manifest
        .core_satisfies(core_version)
        .map_err(|e| BundleError::policy(format!("requires_core is not a semver range: {e}")))?;
    if !satisfied {
        return Err(VerifyFailure::RequiresCore {
            required: signed.manifest.requires_core,
            actual: core_version.to_owned(),
        }
        .into());
    }

    Ok(signed)
}

pub fn read_manifest(archive: &Path) -> BundleResult<SignedBundleManifest> {
    let file = fs::File::open(archive)?;
    let mut tar = Archive::new(GzDecoder::new(file));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() != Path::new(BUNDLE_MANIFEST_FILE) {
            continue;
        }
        let mut raw = String::new();
        entry.read_to_string(&mut raw)?;
        return serde_json::from_str(&raw)
            .map_err(|e| BundleError::policy(format!("bundle.json does not parse: {e}")));
    }
    Err(VerifyFailure::MissingManifest.into())
}

fn verify_signature(signed: &SignedBundleManifest, pinned_keys: &[String]) -> BundleResult<()> {
    if pinned_keys.is_empty() {
        return Ok(());
    }
    let Some(signature) = signed.signature.as_ref() else {
        return Err(VerifyFailure::MissingSignature.into());
    };
    if signature.alg != BUNDLE_SIGNATURE_ALG {
        return Err(VerifyFailure::UnsupportedAlgorithm {
            alg: signature.alg.clone(),
        }
        .into());
    }

    let payload = canonical_manifest_bytes(&signed.manifest)
        .map_err(|e| BundleError::policy(format!("manifest cannot be canonicalised: {e}")))?;

    let matching: Vec<&String> = pinned_keys
        .iter()
        .filter(|k| key_id_for_pubkey(k) == signature.key_id)
        .collect();
    if matching.is_empty() {
        return Err(VerifyFailure::UnknownKey {
            key_id: signature.key_id.clone(),
        }
        .into());
    }
    for key in matching {
        if verify_with_pubkey(key, &payload, &signature.sig_b64).is_ok() {
            return Ok(());
        }
    }
    Err(VerifyFailure::BadSignature.into())
}

pub fn verify_extracted(root: &Path, manifest: &ServicesBundleManifest) -> BundleResult<()> {
    for entry in &manifest.files {
        let path = root.join(&entry.path);
        let content = fs::read(&path).map_err(|_e| VerifyFailure::FileChecksum {
            path: entry.path.clone(),
        })?;
        let digest = hex::encode(Sha256::digest(&content));
        if digest != entry.sha256 || content.len() as u64 != entry.size {
            return Err(VerifyFailure::FileChecksum {
                path: entry.path.clone(),
            }
            .into());
        }
    }

    let declared: BTreeSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    let present = collect_files(root, BUNDLE_ALLOWED_DIRS)?;
    let extra = present
        .iter()
        .filter(|f| !declared.contains(f.path.as_str()))
        .count();
    if extra > 0 {
        return Err(VerifyFailure::UnexpectedFiles { count: extra }.into());
    }

    if ServicesBundleManifest::compute_content_hash(&manifest.files) != manifest.content_hash {
        return Err(VerifyFailure::ContentHash.into());
    }
    Ok(())
}

pub fn require_marketplace_only(manifest: &ServicesBundleManifest) -> BundleResult<()> {
    for dir in &manifest.owns.dirs {
        if !systemprompt_models::services::bundle::MARKETPLACE_BUNDLE_DIRS.contains(&dir.as_str()) {
            return Err(VerifyFailure::NotMarketplaceOnly { dir: dir.clone() }.into());
        }
    }
    Ok(())
}
