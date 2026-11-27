//! Manifests, inputs and an applier over a temporary config dir, shared by the
//! tests.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use systemprompt_device::protocol::ids::InstanceKey;
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_harness::projection::{Applier, BundleFiles, Projection, ProjectionInput, plan};
use systemprompt_models::bridge::host::HostKind;

pub(crate) const INSTANCE_URL: &str = "https://sp.example.com";

pub(crate) fn merge(base: &mut Value, patch: Value) {
    if let (Value::Object(base), Value::Object(patch)) = (base, patch) {
        for (key, value) in patch {
            base.insert(key, value);
        }
    }
}

pub(crate) fn manifest(patch: Value) -> Manifest {
    try_manifest(patch).unwrap()
}

pub(crate) fn try_manifest(patch: Value) -> Result<Manifest, serde_json::Error> {
    let mut base = json!({
        "user_id": "u-1",
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z",
        "not_before": "2026-11-01T09:11:31Z",
        "revocations": [],
        "enabled_hosts": [HostKind::Harness.as_str()],
        "managed_mcp_servers": [],
        "plugins": [],
        "marketplaces": [
            { "id": "alpha-local" },
            { "id": "astound-commons" },
        ],
    });
    merge(&mut base, patch);
    complete(&mut base);
    Manifest::parse(&base.to_string())
}

const NO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn default_key(entry: &mut Value, key: &str, value: Value) {
    if let Value::Object(entry) = entry {
        entry.entry(key).or_insert(value);
    }
}

fn entries<'a>(manifest: &'a mut Value, list: &str) -> impl Iterator<Item = &'a mut Value> {
    manifest
        .get_mut(list)
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
}

/// Gives each skill, rule, plugin and marketplace the bookkeeping fields a
/// manifest must carry and a test leaves out: where the entry came from, its
/// digest, a rule's description, a plugin's version and files, a file's size, a
/// marketplace's name and plugins.
fn complete(manifest: &mut Value) {
    for list in ["skills", "rules"] {
        for entry in entries(manifest, list) {
            default_key(entry, "file_path", json!(""));
            default_key(entry, "sha256", json!(NO_DIGEST));
        }
    }
    for rule in entries(manifest, "rules") {
        default_key(rule, "description", json!(""));
    }
    for marketplace in entries(manifest, "marketplaces") {
        let name = marketplace.get("id").cloned().unwrap_or_default();
        default_key(marketplace, "name", name);
        default_key(marketplace, "plugin_ids", json!([]));
    }
    for plugin in entries(manifest, "plugins") {
        default_key(plugin, "version", json!("0.0.0"));
        default_key(plugin, "sha256", json!(NO_DIGEST));
        default_key(plugin, "files", json!([]));
        for file in entries(plugin, "files") {
            default_key(file, "size", json!(0));
        }
    }
}

pub(crate) fn skill(id: &str, patch: Value) -> Value {
    let mut base = json!({
        "id": id,
        "name": id,
        "description": "Plain description.",
        "instructions": "# Body\n\nDo it.\n",
    });
    merge(&mut base, patch);
    base
}

pub(crate) fn key() -> InstanceKey {
    InstanceKey::new("alpha")
}

pub(crate) fn input<'a>(
    manifest: &'a Manifest,
    files: &'a BundleFiles,
    key: &'a InstanceKey,
) -> ProjectionInput<'a> {
    ProjectionInput {
        manifest,
        profile: None,
        catalog: None,
        instance_url: INSTANCE_URL,
        instance_key: key,
        host: HostKind::Harness,
        bundle_files: files,
    }
}

pub(crate) fn projected(manifest: &Manifest, files: &BundleFiles) -> Projection {
    let key = key();
    plan(&input(manifest, files, &key))
}

pub(crate) fn applier(root: &Path) -> Applier {
    let dir = |name: &str| root.join(name);
    let paths = Paths {
        home: dir("home"),
        data: dir("data"),
        config: dir("config"),
        cache: dir("cache"),
        state: dir("state"),
        tmp: dir("tmp"),
    };
    Applier::new(paths, Layout::default())
}

pub(crate) fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}
