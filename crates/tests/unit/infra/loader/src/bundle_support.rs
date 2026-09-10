//! Shared fixtures for the services-bundle tests: a small services tree, and
//! a packer that can sign, tamper with, or leave a bundle unsigned.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_loader::bundle::pack::{build_manifest, write_tarball};
use systemprompt_models::services::bundle::{
    BUNDLE_SIGNATURE_ALG, BundleSignature, BundleSourceInfo, SignedBundleManifest,
};
use systemprompt_security::manifest_signing::{
    canonical_manifest_bytes, key_id_for_pubkey, pubkey_b64_from_seed, sign_with_seed,
};

pub const SEED: [u8; 32] = [7u8; 32];
pub const OTHER_SEED: [u8; 32] = [9u8; 32];

pub fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, body).expect("write fixture");
}

pub fn base_tree(root: &Path) {
    write(root, "config/config.yaml", "version: 1\n");
    write(root, "access-control/rules.yaml", "rules: []\n");
    write(root, "marketplaces/base/config.yaml", "id: base\n");
}

pub fn marketplace_tree(root: &Path, marketplace: &str, plugin: &str) {
    write(
        root,
        &format!("marketplaces/{marketplace}/config.yaml"),
        &format!("id: {marketplace}\n"),
    );
    write(
        root,
        &format!("plugins/{plugin}/config.yaml"),
        &format!("id: {plugin}\n"),
    );
}

pub fn pubkey() -> String {
    pubkey_b64_from_seed(&SEED)
}

pub struct Packed {
    pub archive: PathBuf,
    pub signed: SignedBundleManifest,
}

pub fn pack(root: &Path, out: &Path, version: &str, requires_core: &str) -> Packed {
    pack_with(root, out, version, requires_core, Some(&SEED))
}

pub fn pack_with(
    root: &Path,
    out: &Path,
    version: &str,
    requires_core: &str,
    seed: Option<&[u8; 32]>,
) -> Packed {
    let manifest = build_manifest(root, version, requires_core, BundleSourceInfo::default())
        .expect("manifest");
    let signature = seed.map(|seed| {
        let payload = canonical_manifest_bytes(&manifest).expect("canonicalise");
        BundleSignature {
            alg: BUNDLE_SIGNATURE_ALG.to_owned(),
            key_id: key_id_for_pubkey(&pubkey_b64_from_seed(seed)),
            sig_b64: sign_with_seed(seed, &payload),
        }
    });
    let signed = SignedBundleManifest {
        manifest,
        signature,
    };
    write_tarball(root, &signed, out).expect("write tarball");
    Packed {
        archive: out.to_path_buf(),
        signed,
    }
}
