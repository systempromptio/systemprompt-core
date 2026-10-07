//! The marketplace sidecar's `claude_code` block: imported into the
//! marketplace config, refused when malformed.

use std::path::Path;

use systemprompt_manifest::services::{ClaudeCodeMarketplaceConfig, MarketplaceConfigFile};
use systemprompt_marketplace::{ImportOptions, import_anthropic_tree};
use tempfile::TempDir;

fn tree(sidecar: Option<&str>) -> TempDir {
    let root = TempDir::new().expect("tempdir");
    let write = |rel: &str, body: &str| {
        let path = root.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write");
    };
    write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","owner":{"name":"Acme"},"plugins":[{"name":"alpha"}]}"#,
    );
    write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha"}"#,
    );
    write(
        "plugins/alpha/skills/a-skill/SKILL.md",
        "---\nname: a-skill\ndescription: What it does.\n---\n\nBody.\n",
    );
    if let Some(body) = sidecar {
        write(".claude-plugin/systemprompt.yaml", body);
    }
    root
}

fn import(source: &Path) -> Result<TempDir, String> {
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source,
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .map(|_| dest)
    .map_err(|e| e.to_string())
}

fn marketplace_config(dest: &Path) -> MarketplaceConfigFile {
    let text = std::fs::read_to_string(dest.join("marketplaces/acme/config.yaml"))
        .expect("marketplace config written");
    serde_yaml::from_str(&text).expect("config parses")
}

#[test]
fn import_carries_the_skill_listing_budget_into_the_marketplace_config() {
    let source = tree(Some(
        "schema: 1\nmarketplace:\n  claude_code:\n    skill_listing_budget_chars: 50000\n",
    ));

    let dest = import(source.path()).expect("import succeeds");

    let text = std::fs::read_to_string(dest.path().join("marketplaces/acme/config.yaml"))
        .expect("written");
    assert!(text.contains("skill_listing_budget_chars: 50000"), "{text}");
    assert_eq!(
        marketplace_config(dest.path()).marketplace.claude_code,
        Some(ClaudeCodeMarketplaceConfig {
            skill_listing_budget_chars: Some(50_000)
        })
    );
}

#[test]
fn import_without_a_claude_code_block_writes_none() {
    let source = tree(Some("schema: 1\nmarketplace:\n  title: Acme\n"));

    let dest = import(source.path()).expect("import succeeds");

    let text = std::fs::read_to_string(dest.path().join("marketplaces/acme/config.yaml"))
        .expect("written");
    assert!(!text.contains("claude_code"), "{text}");
    assert_eq!(
        marketplace_config(dest.path()).marketplace.claude_code,
        None
    );
}

#[test]
fn import_refuses_an_unknown_claude_code_key() {
    let source = tree(Some(
        "schema: 1\nmarketplace:\n  claude_code:\n    skill_listing_budget: 50000\n",
    ));

    let message = import(source.path()).expect_err("unknown key must be refused");

    assert!(message.contains("skill_listing_budget"), "{message}");
}

#[test]
fn import_refuses_a_zero_skill_listing_budget() {
    let source = tree(Some(
        "schema: 1\nmarketplace:\n  claude_code:\n    skill_listing_budget_chars: 0\n",
    ));

    let message = import(source.path()).expect_err("zero budget must be refused");

    assert!(message.contains("skill_listing_budget_chars"), "{message}");
}

#[test]
fn a_services_marketplace_config_accepts_the_same_key() {
    let yaml = "marketplace:\n  id: acme\n  name: Acme\n  description: d\n  version: 1.0.0\n  \
                author:\n    name: Acme\n    email: a@acme.invalid\n  license: proprietary\n  claude_code:\n    \
                skill_listing_budget_chars: 42000\n";

    let file: MarketplaceConfigFile = serde_yaml::from_str(yaml).expect("parses");

    assert_eq!(
        file.marketplace
            .claude_code
            .and_then(|c| c.skill_listing_budget_chars),
        Some(42_000)
    );
    file.marketplace.validate("acme").expect("valid");
}
