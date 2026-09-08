use std::fs;
use std::path::Path;

use systemprompt_bridge::config::paths::LAST_SYNC_SENTINEL;
use systemprompt_bridge::gui::server_marketplace::{build_listing, listing_to_value};
use systemprompt_bridge::mcp_registry;
use systemprompt_bridge::proxy::LoopbackEndpoint;

struct Sandbox {
    root: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            root: tempfile::TempDir::new().expect("sandbox tempdir"),
        }
    }

    fn org_plugins(&self) -> std::path::PathBuf {
        self.root.path().join("org-plugins")
    }

    fn metadata(&self) -> std::path::PathBuf {
        self.root
            .path()
            .join("state")
            .join("systemprompt-bridge")
            .join("metadata")
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let root = self.root.path();
        temp_env::with_vars(
            [
                ("HOME", Some(root.display().to_string())),
                (
                    "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                    Some(self.org_plugins().display().to_string()),
                ),
                (
                    "XDG_CONFIG_HOME",
                    Some(root.join("config").display().to_string()),
                ),
                (
                    "XDG_DATA_HOME",
                    Some(root.join("data").display().to_string()),
                ),
                (
                    "XDG_STATE_HOME",
                    Some(root.join("state").display().to_string()),
                ),
                (
                    "XDG_CACHE_HOME",
                    Some(root.join("cache").display().to_string()),
                ),
            ],
            f,
        )
    }
}

fn healthy_plugin(root: &Path, id: &str) {
    let dir = root.join(id);
    fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
    fs::write(
        dir.join(".claude-plugin").join("plugin.json"),
        r#"{"name": "Healthy Plugin", "description": "It reads"}"#,
    )
    .unwrap();
    fs::create_dir_all(dir.join("skills").join("draft_email")).unwrap();
    fs::write(
        dir.join("skills").join("draft_email").join("SKILL.md"),
        "---\nname: Draft Email\ndescription: Draft things\n---\nBody\n",
    )
    .unwrap();
}

// Why: a directory where a file belongs is the one read failure that needs no
// permission bits, so it behaves the same for an unprivileged runner and root.
fn unreadable_file(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

fn listing_json() -> serde_json::Value {
    let loopback = LoopbackEndpoint::new(9999, None);
    let registry = mcp_registry::snapshot(&mcp_registry::empty_slot());
    let listing = build_listing(&loopback, &registry, &[]).expect("the listing builds");
    listing_to_value(&listing).expect("the listing serialises")
}

fn item<'a>(items: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    items
        .as_array()
        .expect("a category is an array")
        .iter()
        .find(|i| i["id"] == serde_json::json!(id))
        .unwrap_or_else(|| panic!("{id} is missing from {items}"))
}

#[test]
fn a_plugin_whose_manifest_cannot_be_parsed_is_listed_with_an_error_beside_its_healthy_siblings() {
    let sb = Sandbox::new();
    let root = sb.org_plugins();
    healthy_plugin(&root, "healthy-plugin");
    fs::create_dir_all(root.join("broken-plugin").join(".claude-plugin")).unwrap();
    fs::write(
        root.join("broken-plugin")
            .join(".claude-plugin")
            .join("plugin.json"),
        "{ this is not json",
    )
    .unwrap();

    let json = sb.run(listing_json);

    let broken = item(&json["plugins"], "broken-plugin");
    assert!(
        broken["error"].as_str().is_some_and(|e| !e.is_empty()),
        "the unreadable plugin carries the reason it could not be read: {broken}"
    );
    let healthy = item(&json["plugins"], "healthy-plugin");
    assert!(
        healthy.get("error").is_none(),
        "one broken plugin must not taint its siblings: {healthy}"
    );
    assert_eq!(
        healthy["name"],
        serde_json::json!("Healthy Plugin"),
        "the healthy plugin is still fully read"
    );
}

#[test]
fn a_skill_whose_body_cannot_be_read_is_listed_with_an_error_and_the_others_still_appear() {
    let sb = Sandbox::new();
    let root = sb.org_plugins();
    healthy_plugin(&root, "healthy-plugin");
    unreadable_file(
        &root
            .join("healthy-plugin")
            .join("skills")
            .join("broken_skill")
            .join("SKILL.md"),
    );

    let json = sb.run(listing_json);

    let broken = item(&json["skills"], "broken_skill");
    assert!(
        broken["error"].as_str().is_some_and(|e| !e.is_empty()),
        "the skill that could not be read says so: {broken}"
    );
    let healthy = item(&json["skills"], "draft_email");
    assert!(
        healthy.get("error").is_none(),
        "the readable skill is unaffected: {healthy}"
    );
    assert_eq!(healthy["name"], serde_json::json!("Draft Email"));
}

#[test]
fn a_corrupt_last_sync_sentinel_annotates_the_listing_without_hiding_the_plugins() {
    let sb = Sandbox::new();
    healthy_plugin(&sb.org_plugins(), "healthy-plugin");
    fs::create_dir_all(sb.metadata()).unwrap();
    fs::write(sb.metadata().join(LAST_SYNC_SENTINEL), "{ not a sentinel").unwrap();

    let json = sb.run(listing_json);

    assert!(
        json["last_sync_error"]
            .as_str()
            .is_some_and(|e| e.contains(LAST_SYNC_SENTINEL)),
        "the corrupt sentinel is reported and names itself: {json}"
    );
    let healthy = item(&json["plugins"], "healthy-plugin");
    assert!(
        healthy.get("error").is_none(),
        "the plugins on disk are still browsable: {healthy}"
    );
}

#[test]
fn a_readable_tree_reports_no_last_sync_error_at_all() {
    let sb = Sandbox::new();
    healthy_plugin(&sb.org_plugins(), "healthy-plugin");

    let json = sb.run(listing_json);

    assert!(
        json.get("last_sync_error").is_none(),
        "with nothing wrong the key is absent, so the annotation means something: {json}"
    );
}
