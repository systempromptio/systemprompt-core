//! Manifests, inputs and an applier over a temporary config dir, shared by the
//! tests.

use std::fs;
use std::path::Path;

use harness_config::{Layout, Paths};
use serde_json::{Value, json};
use sp_projection::{Applier, BundleFiles, Projection, ProjectionInput, plan};
use sp_protocol::HOST_ID;
use sp_protocol::ids::InstanceKey;
use sp_protocol::wire::manifest::Manifest;

pub(crate) const INSTANCE_URL: &str = "https://sp.example.com";

pub(crate) fn merge(base: &mut Value, patch: Value) {
    if let (Value::Object(base), Value::Object(patch)) = (base, patch) {
        for (key, value) in patch {
            base.insert(key, value);
        }
    }
}

pub(crate) fn manifest(patch: Value) -> Manifest {
    let mut base = json!({
        "user_id": "u-1",
        "enabled_hosts": [HOST_ID],
        "managed_mcp_servers": [],
        "plugins": [],
        "marketplaces": [{ "id": "alpha-local" }, { "id": "astound-commons" }],
    });
    merge(&mut base, patch);
    serde_json::from_value(base).unwrap()
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
        host: HOST_ID,
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
