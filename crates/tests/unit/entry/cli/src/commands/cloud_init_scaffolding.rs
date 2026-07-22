//! Tests for `cloud init` services boilerplate generation against a temp
//! project root. The bundled MCP server directory is pre-created so the git
//! clone step short-circuits without network access.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::cloud::init::scaffolding::generate_services_boilerplate;

fn generate(root: &Path) {
    std::fs::create_dir_all(root.join("services/mcp/systemprompt-admin")).unwrap();
    generate_services_boilerplate(root, "coverage-project").expect("boilerplate generated");
}

#[test]
fn generates_full_services_tree() {
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path());
    let services = dir.path().join("services");

    for file in [
        "config/config.yaml",
        "agents/assistant.yaml",
        "agents/admin.yaml",
        "mcp/systemprompt-admin.yaml",
        "ai/config.yaml",
        "content/config.yaml",
        "web/config.yaml",
        "web/metadata.yaml",
        "scheduler/config.yaml",
        "web/templates/page.html",
        "web/templates/blog-post.html",
        "web/templates/blog-list.html",
        "web/templates/page-list.html",
        "content/blog/welcome/index.md",
        "content/legal/privacy-policy.md",
        "content/legal/cookie-policy.md",
        "skills/.gitkeep",
    ] {
        assert!(services.join(file).is_file(), "missing {file}");
    }

    for yaml in [
        "config/config.yaml",
        "agents/assistant.yaml",
        "ai/config.yaml",
        "web/config.yaml",
        "scheduler/config.yaml",
    ] {
        let raw = std::fs::read_to_string(services.join(yaml)).unwrap();
        serde_yaml::from_str::<serde_yaml::Value>(&raw).unwrap_or_else(|e| {
            panic!("{yaml} is not valid yaml: {e}");
        });
    }

    let web_config = std::fs::read_to_string(services.join("web/config.yaml")).unwrap();
    assert!(web_config.contains("coverage-project"), "{web_config}");

    let gitignore = std::fs::read_to_string(dir.path().join("logs/.gitignore")).unwrap();
    assert!(gitignore.contains("*.log"), "{gitignore}");
}

#[test]
fn rerun_over_existing_tree_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path());
    generate(dir.path());
    assert!(dir.path().join("services/config/config.yaml").is_file());
}
