use std::path::Path;

use systemprompt_identifiers::{AgentId, AgentName, PluginId};
use systemprompt_marketplace::bundle::{BundleContent, build_plugin_bundle, bundle_has_content};
use systemprompt_marketplace::catalog::{load_plugins, plugin_bundles};
use systemprompt_models::bridge::ids::{Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{AgentEntry, SkillEntry};
use systemprompt_models::bridge::plugin_bundle::{
    PLUGIN_MANIFEST_RELPATH, PluginManifest, bundle_has_manifest,
};
use systemprompt_models::services::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig, ServicesConfig,
};

use crate::helpers::{config_with, include, marketplace};

fn zero_digest() -> Sha256Digest {
    Sha256Digest::try_new("0".repeat(64)).expect("zero digest is valid hex")
}

fn skill_entry(id: &str, description: &str, instructions: &str) -> SkillEntry {
    SkillEntry {
        id: SkillId::try_new(id).expect("skill id"),
        name: SkillName::try_new(id.replace('_', " ")).expect("skill name"),
        description: description.to_owned(),
        file_path: format!("/nonexistent/skills/{id}/SKILL.md"),
        tags: vec![],
        sha256: zero_digest(),
        instructions: instructions.to_owned(),
    }
}

fn agent_entry(id: &str, description: &str, prompt: Option<&str>) -> AgentEntry {
    AgentEntry {
        id: AgentId::new(id),
        name: AgentName::try_new(id).expect("agent name"),
        display_name: id.to_owned(),
        description: description.to_owned(),
        version: "1.0.0".to_owned(),
        endpoint: String::new(),
        enabled: true,
        is_default: false,
        is_primary: false,
        provider: None,
        model: None,
        mcp_servers: PluginComponentRef::default(),
        skills: PluginComponentRef::default(),
        tags: vec![],
        system_prompt: prompt.map(str::to_owned),
    }
}

fn explicit(ids: &[&str]) -> PluginComponentRef {
    PluginComponentRef {
        source: ComponentSource::Explicit,
        include: ids.iter().map(|s| (*s).to_owned()).collect(),
        ..Default::default()
    }
}

fn plugin_config(id: &str, skills: PluginComponentRef, agents: PluginComponentRef) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: format!("{id} plugin"),
        description: format!("{id} description"),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "test".to_owned(),
            email: "test@example.com".to_owned(),
        },
        keywords: vec![],
        license: "BSL-1.0".to_owned(),
        category: "demo".to_owned(),
        skills,
        agents,
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        scripts: vec![],
    }
}

#[test]
fn build_plugin_bundle_generates_manifest_and_layout() {
    let skills = vec![skill_entry("use_dangerous_secret", "danger", "do not leak")];
    let agents = vec![agent_entry("developer_agent", "the dev", Some("You are dev"))];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let config = plugin_config(
        "demo-plugin",
        explicit(&["use_dangerous_secret"]),
        explicit(&["developer_agent"]),
    );

    let bundle = build_plugin_bundle(&config, &content).expect("build bundle");

    assert!(bundle_has_manifest(bundle.keys().map(String::as_str)));
    assert!(bundle_has_content(&bundle));
    assert!(bundle.contains_key("skills/use-dangerous-secret/SKILL.md"));
    assert!(bundle.contains_key("agents/developer_agent.md"));

    let manifest: PluginManifest =
        serde_json::from_slice(&bundle[PLUGIN_MANIFEST_RELPATH].bytes).expect("parse manifest");
    assert_eq!(manifest.name, "demo-plugin");
    assert_eq!(manifest.description, "demo-plugin description");
    assert!(
        manifest.version.starts_with("1.0.0+"),
        "version should carry a content hash: {}",
        manifest.version
    );
}

#[test]
fn build_plugin_bundle_instance_source_includes_all_minus_exclude() {
    let skills = vec![
        skill_entry("a_skill", "a", "ab"),
        skill_entry("b_skill", "b", "bb"),
    ];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent"),
    };
    let mut config = plugin_config(
        "p",
        PluginComponentRef::default(),
        PluginComponentRef::default(),
    );
    config.skills.exclude = vec!["b_skill".to_owned()];

    let bundle = build_plugin_bundle(&config, &content).expect("build bundle");
    assert!(bundle.contains_key("skills/a-skill/SKILL.md"));
    assert!(!bundle.contains_key("skills/b-skill/SKILL.md"));
}

#[test]
fn build_plugin_bundle_is_deterministic() {
    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent"),
    };
    let config = plugin_config("p", explicit(&["s"]), PluginComponentRef::default());

    let first = build_plugin_bundle(&config, &content).expect("build");
    let second = build_plugin_bundle(&config, &content).expect("build");
    let first_json = &first[PLUGIN_MANIFEST_RELPATH].bytes;
    let second_json = &second[PLUGIN_MANIFEST_RELPATH].bytes;
    assert_eq!(first_json, second_json);
}

#[test]
fn load_plugins_builds_entry_from_spec_without_prebuilt_dir() {
    let skills = vec![skill_entry("use_dangerous_secret", "d", "x")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let mut services = ServicesConfig::default();
    services.plugins.insert(
        "demo".to_owned(),
        plugin_config(
            "demo-plugin",
            explicit(&["use_dangerous_secret"]),
            PluginComponentRef::default(),
        ),
    );

    let entries = load_plugins(&services, &content).expect("load plugins");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id.as_str(), "demo-plugin");
    assert!(!entries[0].files.is_empty());
    assert!(
        entries[0]
            .files
            .iter()
            .any(|f| f.path == PLUGIN_MANIFEST_RELPATH)
    );
}

#[test]
fn load_plugins_skips_spec_with_no_resolvable_content() {
    let content = BundleContent {
        skills: &[],
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let mut services = ServicesConfig::default();
    services.plugins.insert(
        "demo".to_owned(),
        plugin_config(
            "demo-plugin",
            explicit(&["missing_skill"]),
            PluginComponentRef::default(),
        ),
    );

    let entries = load_plugins(&services, &content).expect("load plugins");
    assert!(
        entries.is_empty(),
        "a spec resolving to no content must be skipped, not shipped as a shell"
    );
}

#[test]
fn bundle_has_manifest_detects_contract_path() {
    assert!(bundle_has_manifest([
        PLUGIN_MANIFEST_RELPATH,
        "skills/x/SKILL.md"
    ]));
    assert!(!bundle_has_manifest([
        "skills/x/SKILL.md",
        "agents/y.md"
    ]));
}

#[test]
fn plugin_bundles_skips_content_less_plugin() {
    let content = BundleContent {
        skills: &[],
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let mut services = ServicesConfig::default();
    services.plugins.insert(
        "demo".to_owned(),
        plugin_config(
            "demo-plugin",
            explicit(&["missing_skill"]),
            PluginComponentRef::default(),
        ),
    );

    let bundles = plugin_bundles(&services, &content).expect("plugin bundles");
    assert!(
        bundles.is_empty(),
        "a content-less plugin must be absent from the served map, mirroring the manifest skip"
    );
}

#[test]
fn plugin_bundles_scopes_to_active_marketplace() {
    let skills = vec![
        skill_entry("a_skill", "a", "ab"),
        skill_entry("b_skill", "b", "bb"),
    ];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent"),
    };
    let mut mp = marketplace("only-a");
    mp.plugins = include(&["plugin-a"]);
    let mut services = config_with(vec![mp]);
    services.plugins.insert(
        "a".to_owned(),
        plugin_config("plugin-a", explicit(&["a_skill"]), PluginComponentRef::default()),
    );
    services.plugins.insert(
        "b".to_owned(),
        plugin_config("plugin-b", explicit(&["b_skill"]), PluginComponentRef::default()),
    );

    let bundles = plugin_bundles(&services, &content).expect("plugin bundles");
    let ids: Vec<&str> = bundles.keys().map(systemprompt_models::bridge::ids::PluginId::as_str).collect();
    assert_eq!(
        ids,
        vec!["plugin-a"],
        "a plugin outside the active marketplace must be absent from the served map"
    );
}

#[test]
fn manifest_entries_hash_the_served_bytes() {
    use sha2::{Digest, Sha256};

    let skills = vec![skill_entry("use_dangerous_secret", "danger", "do not leak")];
    let agents = vec![agent_entry("developer_agent", "the dev", Some("You are dev"))];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let mut services = ServicesConfig::default();
    services.plugins.insert(
        "demo".to_owned(),
        plugin_config(
            "demo-plugin",
            explicit(&["use_dangerous_secret"]),
            explicit(&["developer_agent"]),
        ),
    );

    let entries = load_plugins(&services, &content).expect("load plugins");
    let bundles = plugin_bundles(&services, &content).expect("plugin bundles");
    assert_eq!(entries.len(), 1);

    for entry in &entries {
        let bundle = bundles
            .iter()
            .find(|(id, _)| id.as_str() == entry.id.as_str())
            .map(|(_, bundle)| bundle)
            .expect("the manifest entry has a served bundle");
        assert_eq!(
            entry.files.len(),
            bundle.len(),
            "every served file is recorded in the manifest entry"
        );
        for file in &entry.files {
            let served = bundle.get(&file.path).expect("the manifest path is served");
            let digest: String = Sha256::digest(&served.bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(
                file.sha256.as_str(),
                digest,
                "manifest hash matches served bytes for {}",
                file.path
            );
            assert_eq!(file.size, served.bytes.len() as u64);
        }
    }
}
