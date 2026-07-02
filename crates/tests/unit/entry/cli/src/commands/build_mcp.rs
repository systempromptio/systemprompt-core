//! Tests for `build mcp` manifest validation and failure reporting.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::CliConfig;
use systemprompt_cli::build::mcp::{McpArgs, execute_in_root};

fn cfg() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(systemprompt_cli::OutputFormat::Json)
}

fn write_manifest(root: &Path, name: &str, body: &str) {
    let dir = root.join("extensions").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("manifest.yaml"), body).unwrap();
}

#[test]
fn reports_empty_when_no_extensions_exist() {
    let root = tempfile::tempdir().unwrap();
    let out = execute_in_root(McpArgs { release: false }, &cfg(), root.path()).unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert!(json["items"].as_array().unwrap().is_empty());
}

#[test]
fn errors_when_extension_has_no_binary() {
    let root = tempfile::tempdir().unwrap();
    write_manifest(
        root.path(),
        "no-binary",
        "extension:\n  type: mcp\n  name: no-binary\n",
    );
    let err = execute_in_root(McpArgs { release: false }, &cfg(), root.path()).unwrap_err();
    assert!(err.to_string().contains("has no binary defined"));
}

#[test]
fn records_failure_for_unbuildable_submodule() {
    let root = tempfile::tempdir().unwrap();
    write_manifest(
        root.path(),
        "broken",
        "extension:\n  type: mcp\n  name: broken\n  binary: broken-bin\n  build_type: submodule\n",
    );
    let out = execute_in_root(McpArgs { release: false }, &cfg(), root.path()).unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert!(
        items[0]["status"].as_str().unwrap().starts_with("failed"),
        "{items:?}"
    );
    assert_eq!(items[0]["build_type"], "submodule");
}

#[test]
fn disabled_extensions_are_skipped() {
    let root = tempfile::tempdir().unwrap();
    write_manifest(
        root.path(),
        "off",
        "extension:\n  type: mcp\n  name: off\n  binary: off-bin\n  enabled: false\n",
    );
    let out = execute_in_root(McpArgs { release: false }, &cfg(), root.path()).unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert!(json["items"].as_array().unwrap().is_empty());
}
