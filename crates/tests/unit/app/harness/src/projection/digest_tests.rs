use serde_json::json;
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_harness::projection::{BundleFiles, ContentDigest, digest};
use systemprompt_identifiers::PluginId;

use crate::projection::fixtures::{self, skill};

fn of(manifest: &Manifest, files: &BundleFiles) -> ContentDigest {
    let key = fixtures::key();
    digest(&fixtures::input(manifest, files, &key))
}

#[test]
fn a_resigned_identical_manifest_has_the_same_digest() {
    let files = BundleFiles::new();
    let first = fixtures::manifest(json!({
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z",
        "not_before": "2026-11-01T09:11:31Z",
        "skills": [skill("one", json!({}))]
    }));
    let again = fixtures::manifest(json!({
        "manifest_version": "2026-11-02T10:00:00Z-000001a19661c200",
        "issued_at": "2026-11-02T10:00:00Z",
        "not_before": "2026-11-02T09:59:00Z",
        "skills": [skill("one", json!({}))]
    }));
    assert_eq!(of(&first, &files), of(&again, &files));
    assert_eq!(of(&first, &files).as_str().len(), 64);
    assert_eq!(of(&first, &files).to_string(), of(&first, &files).as_str());
}

#[test]
fn content_bundle_bytes_and_instance_change_the_digest() {
    let files = BundleFiles::new();
    let base = fixtures::manifest(json!({ "skills": [skill("one", json!({}))] }));
    let edited = fixtures::manifest(
        json!({ "skills": [skill("one", json!({ "instructions": "# Other\n" }))] }),
    );
    assert_ne!(of(&base, &files), of(&edited, &files));
    let mut delivered = BundleFiles::new();
    delivered.insert(&PluginId::new("p"), "agents/a.md", b"# A".to_vec());
    assert_ne!(of(&base, &files), of(&base, &delivered));
    let key = fixtures::key();
    let mut input = fixtures::input(&base, &files, &key);
    let before = digest(&input);
    input.instance_url = "https://other.example.com";
    assert_ne!(digest(&input), before);
}
