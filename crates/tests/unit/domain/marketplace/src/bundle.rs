use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::{AgentId, AgentName, PluginId, ValidatedUrl};
use systemprompt_marketplace::bundle::{BundleContent, build_plugin_bundle, bundle_has_content};
use systemprompt_marketplace::catalog::{load_plugins, plugin_bundles, plugin_bundles_cached};
use systemprompt_models::bridge::ids::{ManagedMcpServerName, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{AgentEntry, ManagedMcpServer, SkillEntry};
use systemprompt_models::bridge::plugin_bundle::{
    PLUGIN_MANIFEST_RELPATH, PluginManifest, bundle_has_manifest,
};
use systemprompt_models::services::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig, PluginScript, ServicesConfig,
};

use crate::helpers::{config_with, include, marketplace};

static NO_DISABLED: BTreeSet<String> = BTreeSet::new();

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

fn skill_entry_at(id: &str, description: &str, instructions: &str, file_path: &str) -> SkillEntry {
    let mut e = skill_entry(id, description, instructions);
    e.file_path = file_path.to_owned();
    e
}

fn mcp_server(name: &str, url: &str) -> ManagedMcpServer {
    ManagedMcpServer {
        name: ManagedMcpServerName::try_new(name).expect("mcp name"),
        url: ValidatedUrl::try_new(url).expect("mcp url"),
        transport: Some("http".to_owned()),
        headers: None,
        oauth: None,
        tool_policy: None,
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
    let agents = vec![agent_entry(
        "developer_agent",
        "the dev",
        Some("You are dev"),
    )];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
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
        disabled_mcp_servers: &NO_DISABLED,
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
        disabled_mcp_servers: &NO_DISABLED,
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
        disabled_mcp_servers: &NO_DISABLED,
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
        disabled_mcp_servers: &NO_DISABLED,
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
    assert!(!bundle_has_manifest(["skills/x/SKILL.md", "agents/y.md"]));
}

#[test]
fn plugin_bundles_skips_content_less_plugin() {
    let content = BundleContent {
        skills: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
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
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let mut mp = marketplace("only-a");
    mp.plugins = include(&["plugin-a"]);
    let mut services = config_with(vec![mp]);
    services.plugins.insert(
        "a".to_owned(),
        plugin_config(
            "plugin-a",
            explicit(&["a_skill"]),
            PluginComponentRef::default(),
        ),
    );
    services.plugins.insert(
        "b".to_owned(),
        plugin_config(
            "plugin-b",
            explicit(&["b_skill"]),
            PluginComponentRef::default(),
        ),
    );

    let bundles = plugin_bundles(&services, &content).expect("plugin bundles");
    let ids: Vec<&str> = bundles
        .keys()
        .map(systemprompt_models::bridge::ids::PluginId::as_str)
        .collect();
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
    let agents = vec![agent_entry(
        "developer_agent",
        "the dev",
        Some("You are dev"),
    )];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
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

#[test]
fn cached_bundles_match_the_uncached_build_and_track_input_changes() {
    let skills = vec![skill_entry("cache_skill", "cache", "stay cached")];
    let agents = vec![agent_entry("cache_agent", "cacher", Some("You cache"))];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    let mut services = ServicesConfig::default();
    services.plugins.insert(
        "cache".to_owned(),
        plugin_config(
            "cache-plugin",
            explicit(&["cache_skill"]),
            explicit(&["cache_agent"]),
        ),
    );

    let uncached = comparable(&plugin_bundles(&services, &content).expect("uncached bundles"));
    let first = comparable(
        plugin_bundles_cached(&services, &content)
            .expect("first cached")
            .as_ref(),
    );
    let second = comparable(
        plugin_bundles_cached(&services, &content)
            .expect("second cached")
            .as_ref(),
    );
    assert_eq!(
        first, uncached,
        "the cached map must be exactly what plugin_bundles produces"
    );
    assert_eq!(
        first, second,
        "an unchanged fingerprint must yield an identical map"
    );

    services.plugins.insert(
        "extra".to_owned(),
        plugin_config("extra-plugin", explicit(&[]), explicit(&["cache_agent"])),
    );
    let rebuilt = comparable(
        plugin_bundles_cached(&services, &content)
            .expect("rebuilt cached")
            .as_ref(),
    );
    assert_eq!(
        rebuilt,
        comparable(&plugin_bundles(&services, &content).expect("uncached rebuilt")),
        "a changed services config must invalidate the cache and rebuild"
    );
}

fn comparable(
    bundles: &std::collections::BTreeMap<
        systemprompt_models::bridge::ids::PluginId,
        std::collections::BTreeMap<String, systemprompt_marketplace::bundle::BundleFile>,
    >,
) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<u8>>> {
    bundles
        .iter()
        .map(|(id, files)| {
            let files = files
                .iter()
                .map(|(path, file)| (path.clone(), file.bytes.clone()))
                .collect();
            (id.as_str().to_owned(), files)
        })
        .collect()
}

#[test]
fn skill_md_carries_frontmatter_and_escapes_quotes() {
    let skills = vec![skill_entry(
        "quote_skill",
        "a \"quoted\" desc",
        "  trim me  ",
    )];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let config = plugin_config(
        "p",
        explicit(&["quote_skill"]),
        PluginComponentRef::default(),
    );

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    let md = String::from_utf8(bundle["skills/quote-skill/SKILL.md"].bytes.clone())
        .expect("utf8 SKILL.md");
    assert_eq!(
        md, "---\nname: quote-skill\ndescription: \"a \\\"quoted\\\" desc\"\n---\n\ntrim me\n",
        "SKILL.md must carry escaped description and trimmed instructions",
    );
}

#[test]
fn agent_referenced_skills_are_pulled_into_bundle() {
    let skills = vec![
        skill_entry("base_skill", "b", "body"),
        skill_entry("agent_skill", "a", "agent body"),
    ];
    let mut agent = agent_entry("dev", "dev agent", None);
    agent.skills = explicit(&["agent_skill"]);
    let agents = vec![agent];
    let content = BundleContent {
        skills: &skills,
        agents: &agents,
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let config = plugin_config("p", explicit(&["base_skill"]), explicit(&["dev"]));

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    assert!(
        bundle.contains_key("skills/base-skill/SKILL.md"),
        "explicitly-included skill present",
    );
    assert!(
        bundle.contains_key("skills/agent-skill/SKILL.md"),
        "skill referenced via a selected agent is pulled in",
    );
}

#[test]
fn invalid_explicit_skill_id_is_ignored() {
    let skills = vec![skill_entry("good_skill", "g", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let config = plugin_config(
        "p",
        explicit(&["good_skill", ""]),
        PluginComponentRef::default(),
    );

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    assert!(bundle.contains_key("skills/good-skill/SKILL.md"));
    assert!(
        bundle_has_content(&bundle),
        "an invalid id is skipped without aborting the bundle",
    );
}

#[test]
fn aux_files_are_collected_and_executable_bit_set() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("aux-skill");
    let scripts_dir = skill_dir.join("scripts");
    let nested = scripts_dir.join("nested");
    std::fs::create_dir_all(&nested).expect("create dirs");
    std::fs::write(scripts_dir.join("run.sh"), b"#!/bin/sh\necho hi\n").expect("write sh");
    std::fs::write(scripts_dir.join("notes.txt"), b"plain text").expect("write txt");
    std::fs::write(scripts_dir.join("logo.png"), b"\x89PNG binary").expect("write png");
    std::fs::write(scripts_dir.join(".hidden"), b"skip me").expect("write hidden");
    std::fs::write(nested.join("deep.py"), b"print('hi')\n").expect("write py");
    let pycache = scripts_dir.join("__pycache__");
    std::fs::create_dir_all(&pycache).expect("create pycache");
    std::fs::write(pycache.join("x.txt"), b"ignored").expect("write pycache");

    let skill_md_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_md_path, b"body").expect("write skill md");

    let skills = vec![skill_entry_at(
        "aux_skill",
        "aux",
        "body",
        &skill_md_path.to_string_lossy(),
    )];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let config = plugin_config("p", explicit(&["aux_skill"]), PluginComponentRef::default());

    let bundle = build_plugin_bundle(&config, &content).expect("build");

    let sh = bundle
        .get("skills/aux-skill/scripts/run.sh")
        .expect("shell script collected");
    assert!(sh.executable, "sh files are executable");
    assert_eq!(sh.bytes, b"#!/bin/sh\necho hi\n");

    let txt = bundle
        .get("skills/aux-skill/scripts/notes.txt")
        .expect("text file collected");
    assert!(!txt.executable, "txt files are not executable");

    let py = bundle
        .get("skills/aux-skill/scripts/nested/deep.py")
        .expect("nested python collected with relative path");
    assert!(py.executable, "py files are executable");

    assert!(
        !bundle.contains_key("skills/aux-skill/scripts/logo.png"),
        "binary extensions are excluded",
    );
    assert!(
        !bundle.contains_key("skills/aux-skill/scripts/.hidden"),
        "dotfiles are excluded",
    );
    assert!(
        !bundle.keys().any(|k| k.contains("__pycache__")),
        "__pycache__ is skipped",
    );
}

#[test]
fn mcp_file_assembles_referenced_servers_only() {
    let servers = vec![
        mcp_server("alpha", "https://api.example.com/mcp/alpha"),
        mcp_server("beta", "https://api.example.com/mcp/beta"),
    ];
    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &servers,
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let mut config = plugin_config("p", explicit(&["s"]), PluginComponentRef::default());
    config.mcp_servers = PluginComponentRef {
        source: ComponentSource::Explicit,
        include: vec!["alpha".to_owned(), "missing".to_owned()],
        ..Default::default()
    };

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    let mcp = bundle.get(".mcp.json").expect(".mcp.json emitted");
    let value: serde_json::Value = serde_json::from_slice(&mcp.bytes).expect("parse .mcp.json");
    let servers_obj = value
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .expect("mcpServers object");
    assert_eq!(
        servers_obj.len(),
        1,
        "only referenced-and-present servers are emitted",
    );
    let alpha = servers_obj.get("alpha").expect("alpha present");
    assert_eq!(alpha["type"], "http");
    assert_eq!(alpha["url"], "https://api.example.com/mcp/alpha");
    assert!(
        servers_obj.get("beta").is_none(),
        "an un-referenced server is not emitted",
    );
}

#[test]
fn mcp_file_absent_when_no_servers_resolve() {
    let servers = vec![mcp_server("alpha", "https://api.example.com/mcp/alpha")];
    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &servers,
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: Path::new("/nonexistent"),
    };
    let mut config = plugin_config("p", explicit(&["s"]), PluginComponentRef::default());
    config.mcp_servers = PluginComponentRef {
        source: ComponentSource::Explicit,
        include: vec!["missing".to_owned()],
        ..Default::default()
    };

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    assert!(
        !bundle.contains_key(".mcp.json"),
        "no .mcp.json when every referenced server is missing",
    );
}

#[test]
fn mcp_file_omits_defined_but_disabled_server_without_error() {
    let servers = vec![mcp_server("alpha", "https://api.example.com/mcp/alpha")];
    let disabled: BTreeSet<String> = ["salesforce".to_owned()].into_iter().collect();
    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &servers,
        disabled_mcp_servers: &disabled,
        plugins_root: Path::new("/nonexistent"),
    };
    let mut config = plugin_config("p", explicit(&["s"]), PluginComponentRef::default());
    config.mcp_servers = PluginComponentRef {
        source: ComponentSource::Explicit,
        include: vec!["alpha".to_owned(), "salesforce".to_owned()],
        ..Default::default()
    };

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    let mcp = bundle
        .get(".mcp.json")
        .expect(".mcp.json emitted for the enabled server");
    let value: serde_json::Value = serde_json::from_slice(&mcp.bytes).expect("parse .mcp.json");
    let servers_obj = value
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .expect("mcpServers object");
    assert!(
        servers_obj.contains_key("alpha"),
        "enabled server is bundled"
    );
    assert!(
        !servers_obj.contains_key("salesforce"),
        "a defined-but-disabled server is quietly omitted, not bundled",
    );
}

#[test]
fn mcp_file_absent_when_only_referenced_server_is_disabled() {
    let disabled: BTreeSet<String> = ["salesforce".to_owned()].into_iter().collect();
    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &disabled,
        plugins_root: Path::new("/nonexistent"),
    };
    let mut config = plugin_config("p", explicit(&["s"]), PluginComponentRef::default());
    config.mcp_servers = PluginComponentRef {
        source: ComponentSource::Explicit,
        include: vec!["salesforce".to_owned()],
        ..Default::default()
    };

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    assert!(
        !bundle.contains_key(".mcp.json"),
        "a sole disabled reference yields no .mcp.json, but does not error",
    );
}

#[test]
fn script_files_are_collected_and_generated_tracking_skipped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let plugin_dir = dir.path().join("scripted-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    std::fs::write(plugin_dir.join("setup.sh"), b"#!/bin/sh\necho setup\n").expect("write script");

    let skills = vec![skill_entry("s", "d", "body")];
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        plugins_root: dir.path(),
    };
    let mut config = plugin_config(
        "scripted-plugin",
        explicit(&["s"]),
        PluginComponentRef::default(),
    );
    config.scripts = vec![
        PluginScript {
            name: "setup".to_owned(),
            source: "setup.sh".to_owned(),
        },
        PluginScript {
            name: "tracker".to_owned(),
            source: "generated:tracking".to_owned(),
        },
        PluginScript {
            name: "absent".to_owned(),
            source: "does-not-exist.sh".to_owned(),
        },
    ];

    let bundle = build_plugin_bundle(&config, &content).expect("build");
    let setup = bundle
        .get("scripts/setup")
        .expect("on-disk script collected");
    assert!(setup.executable, "plugin scripts are executable");
    assert_eq!(setup.bytes, b"#!/bin/sh\necho setup\n");
    assert!(
        !bundle.contains_key("scripts/tracker"),
        "generated:tracking scripts are synthesised by the consumer, not bundled",
    );
    assert!(
        !bundle.contains_key("scripts/absent"),
        "a missing source file is silently skipped",
    );
}
