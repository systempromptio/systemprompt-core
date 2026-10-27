//! `OpenCode` V2 theme projection: every marketplace theme is copied, the
//! default is selected in `cli.json` unless the user chose one, and the theme
//! key an older bridge wrote into `opencode.json` is removed.

use std::fs;
use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha256};
use systemprompt_bridge::gateway::manifest::{ManifestMarketplace, PluginEntry, SignedManifest};
use systemprompt_bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{ManifestOpenCode, PluginFile, ThemeMode, ThemeRef};

use crate::opencode_managed_resources::{apply, manifest_with, with_sandbox};

const PLUGIN: &str = "brand";
const THEME_BODY: &str = r#"{"theme":{"primary":"#c00"}}"#;

fn digest(bytes: &[u8]) -> Sha256Digest {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Sha256Digest::try_new(hex).expect("sha256 hex")
}

fn ship(root: &Path, rel: &str) -> PluginFile {
    let path = root.join(PLUGIN).join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("plugin dir");
    fs::write(&path, THEME_BODY).expect("theme file");
    PluginFile {
        path: rel.to_owned(),
        sha256: digest(THEME_BODY.as_bytes()),
        size: THEME_BODY.len() as u64,
    }
}

fn themed_manifest(root: &Path, opencode: ManifestOpenCode) -> SignedManifest {
    let files = ["dark", "black"]
        .iter()
        .map(|name| ship(root, &format!("hosts/opencode/themes/{name}.json")))
        .chain(std::iter::once(ship(
            root,
            "hosts/opencode/desktop-themes/dark.json",
        )))
        .collect();
    let mut m = manifest_with(Vec::new(), Vec::new());
    m.plugins = vec![PluginEntry {
        id: PluginId::try_new(PLUGIN).expect("plugin id"),
        version: "1.0.0".into(),
        sha256: digest(b"bundle"),
        files,
        hooks: systemprompt_models::plugin::PluginHooksRef::default(),
        publication: None,
    }];
    m.marketplaces = vec![ManifestMarketplace {
        id: systemprompt_identifiers::MarketplaceId::new("brand"),
        name: "Brand".into(),
        plugin_ids: vec![],
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
        external_plugins: vec![],
        claude_code: None,
        opencode: Some(opencode),
    }];
    m
}

fn two_themes(default: &str, mode: Option<ThemeMode>) -> ManifestOpenCode {
    let sha = digest(THEME_BODY.as_bytes());
    ManifestOpenCode {
        themes: ["dark", "black"]
            .iter()
            .map(|name| ThemeRef {
                name: (*name).to_owned(),
                sha: sha.clone(),
            })
            .collect(),
        default_theme: Some(default.to_owned()),
        theme_mode: mode,
        ..ManifestOpenCode::default()
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("json")
}

#[test]
fn every_theme_is_projected_and_the_default_is_selected_in_cli_json() {
    with_sandbox(|sb| {
        let org = tempfile::tempdir().expect("org root");
        let m = themed_manifest(org.path(), two_themes("black", Some(ThemeMode::Dark)));
        apply(&m, org.path()).expect("apply");

        let dir = sb.config.parent().expect("opencode dir");
        assert!(dir.join("themes/dark.json").is_file());
        assert!(dir.join("themes/black.json").is_file());
        assert!(dir.join("desktop-themes/dark.json").is_file());
        assert_eq!(
            read_json(&dir.join("cli.json"))["theme"],
            json!({"name": "black", "mode": "dark"})
        );
        assert!(
            !sb.config.exists() || read_json(&sb.config).get("theme").is_none(),
            "the V2 opencode.json schema has no theme key"
        );
    });
}

#[test]
fn a_theme_the_user_chose_is_never_replaced() {
    with_sandbox(|sb| {
        let org = tempfile::tempdir().expect("org root");
        let dir = sb.config.parent().expect("opencode dir");
        fs::create_dir_all(dir).expect("opencode dir");
        fs::write(dir.join("cli.json"), r#"{"theme":{"name":"oc-2"}}"#).expect("cli.json");

        apply(&themed_manifest(org.path(), two_themes("dark", None)), org.path()).expect("apply");

        assert_eq!(read_json(&dir.join("cli.json"))["theme"], json!({"name": "oc-2"}));
    });
}

#[test]
fn the_theme_key_an_older_bridge_wrote_into_opencode_json_is_removed() {
    with_sandbox(|sb| {
        let org = tempfile::tempdir().expect("org root");
        fs::create_dir_all(sb.config.parent().expect("opencode dir")).expect("opencode dir");
        fs::write(&sb.config, r#"{"theme":"dark","autoupdate":false}"#).expect("opencode.json");

        apply(&themed_manifest(org.path(), two_themes("dark", None)), org.path()).expect("apply");

        let doc = read_json(&sb.config);
        assert!(doc.get("theme").is_none(), "{doc}");
        assert_eq!(doc["autoupdate"], json!(false), "{doc}");
    });
}
