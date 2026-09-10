use std::path::{Path, PathBuf};

use systemprompt_loader::ConfigLoader;
use systemprompt_marketplace::{ImportOptions, ImportWarning, import_anthropic_tree};
use tempfile::TempDir;

pub(crate) fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn import_good() -> (TempDir, systemprompt_marketplace::ImportReport) {
    let dest = TempDir::new().expect("tempdir");
    let report = import_anthropic_tree(
        &fixture("anthropic"),
        dest.path(),
        &ImportOptions::default(),
    )
    .expect("import succeeds");
    (dest, report)
}

#[test]
fn report_lists_every_imported_entity() {
    let (_dest, report) = import_good();

    assert_eq!(
        report
            .marketplaces
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>(),
        vec!["acme-field"]
    );
    assert_eq!(
        report
            .plugins
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha-tools", "beta-reports"]
    );
    assert_eq!(
        report.skills,
        vec!["alpha_discovery", "alpha_report", "beta_summary"]
    );
    assert_eq!(report.rules, vec!["handover", "security"]);
    assert_eq!(report.copied_base_dirs, vec!["mcp"]);
    assert_eq!(report.hooks.len(), 2);
}

#[test]
fn imported_tree_loads_through_the_config_loader() {
    let (dest, _report) = import_good();
    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");

    assert_eq!(services.marketplaces.len(), 1);
    assert_eq!(services.plugins.len(), 2);
    assert_eq!(services.skills.skills.len(), 3);
    assert!(services.mcp_servers.contains_key("knowledge-bank"));
}

#[test]
fn marketplace_access_rules_survive_the_sidecar() {
    let (dest, _report) = import_good();
    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");
    let marketplace = services
        .marketplaces
        .values()
        .next()
        .expect("one marketplace");

    assert_eq!(marketplace.access.rules.len(), 1);
    let rule = &marketplace.access.rules[0];
    assert_eq!(rule.rule_type, "group");
    assert_eq!(rule.values, vec!["field"]);
    assert!(!marketplace.access.default_included);
    assert_eq!(marketplace.mcp_servers.include, vec!["knowledge-bank"]);
    assert_eq!(
        marketplace.plugins.include,
        vec!["alpha-tools", "beta-reports"]
    );
    assert_eq!(marketplace.license, "proprietary");
    assert_eq!(marketplace.author.name, "Acme Field Team");
}

#[test]
fn plugin_derives_skills_and_takes_the_rest_from_the_sidecar() {
    let (dest, _report) = import_good();
    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");

    let alpha = services.plugins.get("alpha-tools").expect("alpha-tools");
    assert_eq!(alpha.category, "business");
    assert_eq!(alpha.version, "1.4.0");
    assert_eq!(alpha.mcp_servers.include, vec!["knowledge-bank"]);
    assert_eq!(
        alpha.skills.include,
        vec!["alpha_discovery", "alpha_report"]
    );
    assert_eq!(alpha.rules.include, vec!["handover"]);
    assert_eq!(alpha.hooks.include.len(), 2);
    assert!(
        alpha
            .hooks
            .include
            .iter()
            .any(|h| h == "alpha_tools__PreToolUse__0")
    );
}

#[test]
fn category_falls_back_to_the_marketplace_entry() {
    let (dest, _report) = import_good();
    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");

    let beta = services.plugins.get("beta-reports").expect("beta-reports");
    assert!(beta.rules.include.is_empty());
    assert_eq!(beta.category, "reporting");
    assert!(beta.mcp_servers.include.is_empty());
}

#[test]
fn base_tree_and_skill_files_are_copied_verbatim() {
    let (dest, _report) = import_good();
    let root = dest.path();

    assert!(root.join("mcp/knowledge-bank.yaml").is_file());
    assert!(root.join("skills/alpha_discovery/SKILL.md").is_file());
    assert!(root.join("skills/alpha_discovery/checklist.md").is_file());
    assert!(root.join("skills/alpha_discovery/config.yaml").is_file());
    assert!(root.join("plugins/alpha-tools/scripts/setup.sh").is_file());
    assert!(root.join("rules/security/index.md").is_file());
    assert!(root.join("rules/security/config.yaml").is_file());
    assert!(root.join("rules/handover/index.md").is_file());
    assert!(root.join("rules/handover/config.yaml").is_file());

    let original = std::fs::read(
        fixture("anthropic").join("plugins/alpha-tools/skills/alpha_discovery/SKILL.md"),
    )
    .expect("read original");
    let copied = std::fs::read(root.join("skills/alpha_discovery/SKILL.md")).expect("read copy");
    assert_eq!(original, copied);
}

#[test]
fn a_root_level_rule_belongs_to_no_plugin_and_is_reported() {
    let (_dest, report) = import_good();

    assert!(report.warnings.iter().any(|w| matches!(
        w,
        ImportWarning::UnattachedRootRules { rules } if rules == &vec!["security".to_owned()]
    )));
}

#[test]
fn the_imported_rules_load_through_the_rules_catalogue() {
    let (dest, _report) = import_good();
    let rules = systemprompt_marketplace::catalog::load_rules(dest.path()).expect("rules load");

    assert_eq!(
        rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["handover", "security"]
    );
    assert!(rules[0].instructions.starts_with("A handover names"));
}

#[test]
fn hook_descriptors_carry_the_command_and_matcher() {
    let (dest, _report) = import_good();
    let text = std::fs::read_to_string(
        dest.path()
            .join("hooks/alpha-tools__PreToolUse__0/config.yaml"),
    )
    .expect("hook config written");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("valid yaml");

    assert_eq!(doc["event"].as_str(), Some("PreToolUse"));
    assert_eq!(doc["matcher"].as_str(), Some("Bash"));
    assert_eq!(doc["command"].as_str(), Some("alpha-guard --check"));
}

#[test]
fn a_tree_without_a_marketplace_manifest_still_copies_the_base() {
    let source = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(source.path().join("systemprompt/mcp")).expect("mkdir");
    std::fs::copy(
        fixture("anthropic").join("systemprompt/mcp/knowledge-bank.yaml"),
        source.path().join("systemprompt/mcp/knowledge-bank.yaml"),
    )
    .expect("copy base file");

    let dest = TempDir::new().expect("tempdir");
    let report =
        import_anthropic_tree(source.path(), dest.path(), &ImportOptions::default()).expect("ok");

    assert_eq!(report.copied_base_dirs, vec!["mcp"]);
    assert!(report.marketplaces.is_empty());
    assert!(
        report
            .warnings
            .contains(&ImportWarning::NoMarketplaceManifest)
    );
}

#[test]
fn a_marketplace_only_tree_gets_no_root_config() {
    let source = TempDir::new().expect("source tempdir");
    let status = std::process::Command::new("cp")
        .arg("-r")
        .arg(fixture("anthropic"))
        .arg(source.path().join("repo"))
        .status()
        .expect("copy fixture");
    assert!(status.success());
    let repo = source.path().join("repo");
    std::fs::remove_dir_all(repo.join("systemprompt")).expect("drop the base tree");

    let dest = TempDir::new().expect("tempdir");
    let report = import_anthropic_tree(&repo, dest.path(), &ImportOptions::default())
        .expect("import succeeds");

    assert!(report.copied_base_dirs.is_empty());
    assert!(
        !dest.path().join("config").exists(),
        "a tree that composes over a platform bundle must not claim config/"
    );
    assert!(
        dest.path()
            .join("marketplaces/acme-field/config.yaml")
            .is_file()
    );
}

#[test]
fn sidecar_and_frontmatter_titles_become_display_names() {
    let (dest, _report) = import_good();
    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");

    let marketplace = services
        .marketplaces
        .iter()
        .find(|(id, _)| id.as_str() == "acme-field")
        .map(|(_, m)| m)
        .expect("marketplace");
    assert_eq!(marketplace.name, "Acme Field Engineering");

    let alpha = services.plugins.get("alpha-tools").expect("alpha plugin");
    assert_eq!(alpha.name, "Alpha Tools");
    let beta = services.plugins.get("beta-reports").expect("beta plugin");
    assert_eq!(beta.name, "beta-reports", "no title falls back to the id");

    let skill = dest.path().join("skills/alpha_discovery/config.yaml");
    let text = std::fs::read_to_string(skill).expect("skill config");
    assert!(text.contains("name: Alpha Discovery"), "{text}");
    assert!(
        text.contains("display_category: Field — Discovery"),
        "{text}"
    );
}
