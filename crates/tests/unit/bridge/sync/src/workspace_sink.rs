use std::fs;
use std::path::PathBuf;

use systemprompt_bridge::gateway::manifest::ArtifactEntry;
use systemprompt_bridge::ids::{LibraryArtifactId, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::workspace_sink::{
    BUNDLE_MANIFEST_FILE, bundle_is_current, remove_bundle, write_bundle,
};

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-workspace-sink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn artifact(id: &str, version: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).unwrap(),
        name: format!("name of {id}"),
        description: "desc".into(),
        version: version.to_owned(),
        mcp_tools: vec!["mcp__odoo__crm_lead_search".to_owned()],
        content: format!("<html><body id=\"{id}\"></body></html>"),
        starred: false,
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        plugins: Vec::new(),
    }
}

// Why: the setup skills install from this bundle with file tools only, so a
// page must land verbatim beside a manifest that names it — no shell copy.
#[test]
fn bundle_writes_manifest_and_one_verbatim_page_per_record() {
    let dir = tempdir();
    let set = [artifact("business-overview", "1"), artifact("admin-users-directory", "2")];
    write_bundle(&dir, &set).unwrap();

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(BUNDLE_MANIFEST_FILE)).unwrap()).unwrap();
    let ids: Vec<&str> = manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["business-overview", "admin-users-directory"]);
    assert_eq!(
        manifest["artifacts"][0]["mcpTools"][0],
        "mcp__odoo__crm_lead_search"
    );
    assert!(manifest["artifacts"][0].get("content").is_none());
    assert_eq!(
        fs::read_to_string(dir.join("business-overview.html")).unwrap(),
        set[0].content
    );
    assert!(bundle_is_current(&dir, &set));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_dropped_id_is_removed_and_a_version_change_is_not_current() {
    let dir = tempdir();
    write_bundle(&dir, &[artifact("a", "1"), artifact("b", "1")]).unwrap();
    assert!(dir.join("b.html").is_file());

    let next = [artifact("a", "2")];
    assert!(!bundle_is_current(&dir, &next));
    write_bundle(&dir, &next).unwrap();
    assert!(!dir.join("b.html").exists());
    assert!(bundle_is_current(&dir, &next));

    remove_bundle(&dir).unwrap();
    assert!(!dir.exists());
}

#[test]
fn an_empty_dir_is_never_current() {
    let dir = tempdir();
    assert!(!bundle_is_current(&dir, &[artifact("a", "1")]));
    fs::remove_dir_all(&dir).unwrap();
}
