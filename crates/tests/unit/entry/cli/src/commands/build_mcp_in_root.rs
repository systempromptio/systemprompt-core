//! Tests for `build mcp` driven against a tempdir project root.
//!
//! `execute_in_root` discovers manifests under `<root>/extensions`, so a
//! scaffolded root exercises the empty-registry short circuit, the
//! missing-binary rejection, and both build strategies' failure reporting
//! without needing a real project.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::build::mcp::{McpArgs, execute_in_root};
use systemprompt_cli::{CliConfig, OutputFormat};

fn config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn write_manifest(root: &Path, dir_name: &str, body: &str) {
    let dir = root.join("extensions").join(dir_name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("manifest.yaml"), body).unwrap();
}

fn artifact_text(output: &systemprompt_cli::shared::CommandOutput) -> String {
    serde_json::to_value(output.artifact()).unwrap().to_string()
}

#[test]
fn a_root_without_extensions_produces_an_empty_build_table() {
    let tmp = tempfile::tempdir().unwrap();

    let output = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap();
    let text = artifact_text(&output);

    assert_eq!(output.title(), Some("Build MCP Extensions"));
    assert!(text.contains("\"items\":[]"));
}

#[test]
fn a_non_mcp_extension_is_not_built() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "blogger",
        "extension:\n  type: blog\n  name: blogger\n  binary: blogger\n",
    );

    let output = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap();
    assert!(artifact_text(&output).contains("\"items\":[]"));
}

#[test]
fn a_disabled_mcp_extension_is_not_built() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "sleeping",
        "extension:\n  type: mcp\n  name: sleeping\n  binary: sleeping\n  enabled: false\n",
    );

    let output = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap();
    assert!(artifact_text(&output).contains("\"items\":[]"));
}

#[test]
fn an_mcp_extension_without_a_binary_aborts_the_whole_build() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "nobinary",
        "extension:\n  type: mcp\n  name: nobinary\n",
    );

    let err = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap_err();
    assert!(err.to_string().contains("nobinary"));
    assert!(err.to_string().contains("no binary defined"));
}

#[test]
fn a_workspace_extension_reports_a_missing_root_manifest_as_a_failed_row() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "wsext",
        "extension:\n  type: mcp\n  name: wsext\n  binary: wsext-bin\n  build_type: workspace\n",
    );

    let output = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap();
    let text = artifact_text(&output);

    assert!(text.contains("wsext"));
    assert!(text.contains("workspace"));
    assert!(text.contains("Cargo.toml not found in project root"));
}

#[test]
fn a_workspace_extension_reports_a_cargo_failure_as_a_failed_row() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[workspace]\nmembers = []\nresolver = \"2\"\n",
    )
    .unwrap();
    write_manifest(
        tmp.path(),
        "wsext",
        "extension:\n  type: mcp\n  name: wsext\n  binary: absent_package\n  build_type: \
         workspace\n",
    );

    let output = execute_in_root(McpArgs { release: true }, &config(), tmp.path()).unwrap();
    let text = artifact_text(&output);

    assert!(text.contains("failed: Failed to build absent_package"));
}

#[test]
fn a_submodule_extension_reports_a_cargo_failure_as_a_failed_row() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "subext",
        "extension:\n  type: mcp\n  name: subext\n  binary: subext-bin\n  build_type: submodule\n",
    );

    let output = execute_in_root(McpArgs { release: false }, &config(), tmp.path()).unwrap();
    let text = artifact_text(&output);

    assert!(text.contains("submodule"));
    assert!(text.contains("failed: Failed to build subext"));
}

#[test]
fn text_output_mode_renders_the_same_rows() {
    let tmp = tempfile::tempdir().unwrap();
    write_manifest(
        tmp.path(),
        "wsext",
        "extension:\n  type: mcp\n  name: wsext\n  binary: wsext-bin\n",
    );

    let output = execute_in_root(
        McpArgs { release: false },
        &CliConfig::new().with_interactive(false),
        tmp.path(),
    )
    .unwrap();

    assert!(artifact_text(&output).contains("Cargo.toml not found in project root"));
}
