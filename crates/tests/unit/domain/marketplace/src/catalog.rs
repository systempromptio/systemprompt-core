use std::collections::HashMap;
use std::fs;

use systemprompt_marketplace::catalog::{
    disabled_mcp_server_names, load_agents, load_artifacts, load_hooks, load_managed_mcp_servers,
    load_plugins, load_skills,
};
use systemprompt_marketplace::{BundleContent, CatalogContent};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::OAuthRequirement;
use systemprompt_models::mcp::{Deployment, ExternalAuth, McpServerType};
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, OAuthConfig, ServicesConfig,
};

use crate::helpers::config_with;

fn make_agent_config(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_owned(),
        port: 8080,
        endpoint: String::new(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: vec![],
        card: AgentCardConfig {
            protocol_version: "0.2.5".into(),
            name: Some(name.to_owned()),
            display_name: name.to_owned(),
            description: format!("{name} agent"),
            version: "1.0.0".into(),
            preferred_transport: "http".into(),
            icon_url: None,
            documentation_url: None,
            provider: None,
            capabilities: Default::default(),
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
    }
}

fn make_deployment(_name: &str, enabled: bool, endpoint: Option<&str>) -> Deployment {
    Deployment {
        server_type: Default::default(),
        binary: "server".into(),
        package: None,
        port: 3000,
        endpoint: endpoint.map(ToOwned::to_owned),
        enabled,
        display_in_web: true,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: Default::default(),
    }
}

#[test]
fn load_agents_empty_config_returns_empty() {
    let config = config_with(vec![]);
    let agents = load_agents(&config, "https://api.example.com");
    assert!(agents.is_empty());
}

#[test]
fn load_agents_single_enabled_agent() {
    let mut config = ServicesConfig::default();
    config
        .agents
        .insert("my-agent".to_owned(), make_agent_config("my-agent"));
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].id.as_str(), "my-agent");
}

#[test]
fn load_agents_disabled_agents_excluded() {
    let mut config = ServicesConfig::default();
    let mut disabled = make_agent_config("disabled-agent");
    disabled.enabled = false;
    config.agents.insert("disabled-agent".to_owned(), disabled);
    let agents = load_agents(&config, "https://api.example.com");
    assert!(agents.is_empty());
}

#[test]
fn load_agents_endpoint_built_from_base_when_empty() {
    let mut config = ServicesConfig::default();
    let agent = make_agent_config("search");
    config.agents.insert("search".to_owned(), agent);
    let agents = load_agents(&config, "https://api.example.com/");
    assert_eq!(agents.len(), 1);
    assert!(
        agents[0].endpoint.starts_with("https://api.example.com"),
        "endpoint must be derived from base url",
    );
}

#[test]
fn load_agents_absolute_endpoint_passed_through() {
    let mut config = ServicesConfig::default();
    let mut agent = make_agent_config("remote");
    agent.endpoint = "https://remote.example.com/a2a".into();
    config.agents.insert("remote".to_owned(), agent);
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents[0].endpoint, "https://remote.example.com/a2a");
}

#[test]
fn load_agents_sorted_alphabetically() {
    let mut config = ServicesConfig::default();
    config
        .agents
        .insert("bravo".to_owned(), make_agent_config("bravo"));
    config
        .agents
        .insert("alpha".to_owned(), make_agent_config("alpha"));
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents[0].id.as_str(), "alpha");
    assert_eq!(agents[1].id.as_str(), "bravo");
}

#[test]
fn load_skills_no_skills_dir_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let result = load_skills(dir.path()).expect("no error when skills dir absent");
    assert!(result.is_empty());
}

#[test]
fn load_skills_dir_with_valid_skill() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("my-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: my_skill\nname: My Skill\ndescription: test\nenabled: true\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id.as_str(), "my_skill");
}

#[test]
fn load_skills_disabled_skill_excluded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("off-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: off_skill\nname: Off\ndescription: disabled\nenabled: false\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert!(skills.is_empty());
}

#[test]
fn load_skills_sorted_alphabetically() {
    let dir = tempfile::tempdir().expect("temp dir");
    for name in &["zebra", "apple"] {
        let skill_dir = dir.path().join("skills").join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("config.yaml"),
            format!("id: {name}\nname: {name}\ndescription: test\nenabled: true\n"),
        )
        .expect("write config");
    }
    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills[0].id.as_str(), "apple");
    assert_eq!(skills[1].id.as_str(), "zebra");
}

#[test]
fn load_skills_reads_content_file_strips_frontmatter_and_hashes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("my-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: my_skill\nname: My Skill\ndescription: a desc\nenabled: true\ntags:\n  - alpha\n  - beta\n",
    )
    .expect("write config");
    fs::write(
        skill_dir.join("index.md"),
        "---\ntitle: ignored\n---\nReal body here.\n",
    )
    .expect("write content");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    let s = &skills[0];
    assert_eq!(s.instructions, "Real body here.", "frontmatter stripped");
    assert_eq!(s.description, "a desc");
    assert_eq!(s.tags, vec!["alpha".to_owned(), "beta".to_owned()]);
    assert!(
        s.file_path.ends_with("index.md"),
        "file_path points at the content file",
    );

    use sha2::{Digest, Sha256};
    let expected: String = Sha256::digest(s.instructions.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        s.sha256.as_str(),
        expected,
        "sha256 hashes the stripped instructions",
    );
}

#[test]
fn load_skills_empty_name_derives_display_from_dir() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("my_named_skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: my_named_skill\nname: \"\"\ndescription: d\nenabled: true\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    assert_eq!(
        skills[0].name.as_str(),
        "my named skill",
        "an empty name falls back to the dir name with underscores spaced",
    );
}

#[test]
fn load_skills_missing_content_file_yields_empty_instructions() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("bare-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: bare_skill\nname: Bare\ndescription: d\nenabled: true\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    assert_eq!(
        skills[0].instructions, "",
        "absent content file yields empty instructions",
    );
}

#[test]
fn load_skills_invalid_config_is_skipped_not_fatal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let good = dir.path().join("skills").join("good");
    let bad = dir.path().join("skills").join("bad");
    fs::create_dir_all(&good).expect("create good");
    fs::create_dir_all(&bad).expect("create bad");
    fs::write(
        good.join("config.yaml"),
        "id: good\nname: Good\ndescription: d\nenabled: true\n",
    )
    .expect("write good config");
    fs::write(bad.join("config.yaml"), "this: [is, not, valid").expect("write bad config");

    let skills = load_skills(dir.path()).expect("an unparseable skill is skipped, not fatal");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id.as_str(), "good");
}

#[test]
fn load_skills_custom_content_file_honoured() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("custom");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: custom\nname: Custom\ndescription: d\nenabled: true\nfile: PROMPT.md\n",
    )
    .expect("write config");
    fs::write(skill_dir.join("PROMPT.md"), "custom body\n").expect("write custom content");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].instructions, "custom body\n");
    assert!(skills[0].file_path.ends_with("PROMPT.md"));
}

#[test]
fn load_hooks_no_hooks_dir_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let result = load_hooks(dir.path()).expect("no error when hooks dir absent");
    assert!(result.is_empty());
}

#[test]
fn load_hooks_dir_with_valid_hook() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hook_dir = dir.path().join("hooks").join("my-hook");
    fs::create_dir_all(&hook_dir).expect("create hook dir");
    fs::write(
        hook_dir.join("config.yaml"),
        "event: PreToolUse\nenabled: true\ncommand: echo hello\n",
    )
    .expect("write config");

    let hooks = load_hooks(dir.path()).expect("load hooks");
    assert_eq!(hooks.len(), 1);
}

#[test]
fn load_hooks_disabled_hook_excluded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hook_dir = dir.path().join("hooks").join("off-hook");
    fs::create_dir_all(&hook_dir).expect("create hook dir");
    fs::write(
        hook_dir.join("config.yaml"),
        "event: PostToolUse\nenabled: false\ncommand: echo off\n",
    )
    .expect("write config");

    let hooks = load_hooks(dir.path()).expect("load hooks");
    assert!(hooks.is_empty());
}

#[test]
fn load_managed_mcp_servers_empty_config_returns_empty() {
    let config = ServicesConfig::default();
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(servers.is_empty());
}

#[test]
fn load_managed_mcp_servers_single_enabled() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("my-mcp".to_owned(), make_deployment("my-mcp", true, None));
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name.as_str(), "my-mcp");
}

#[test]
fn load_managed_mcp_servers_disabled_excluded() {
    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(
        "off-mcp".to_owned(),
        make_deployment("off-mcp", false, None),
    );
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(servers.is_empty());
}

#[test]
fn load_managed_mcp_servers_default_endpoint_synthesised() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("my-mcp".to_owned(), make_deployment("my-mcp", true, None));
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(
        servers[0].url.as_str().contains("my-mcp"),
        "synthesised url must contain server name",
    );
}

#[test]
fn load_managed_mcp_servers_absolute_endpoint_used() {
    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(
        "remote-mcp".to_owned(),
        make_deployment("remote-mcp", true, Some("https://remote.example.com/mcp")),
    );
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(servers[0].url.as_str(), "https://remote.example.com/mcp",);
}

#[test]
fn load_managed_mcp_servers_external_with_accessor_routes_through_gateway() {
    let mut config = ServicesConfig::default();
    let mut dep = make_deployment("sf", true, Some("https://api.salesforce.com/mcp"));
    dep.server_type = McpServerType::External;
    dep.external_auth = Some(ExternalAuth {
        token_endpoint: "/api/public/sf/token".to_owned(),
        header: "Authorization".to_owned(),
        scheme: "Bearer".to_owned(),
    });
    config.mcp_servers.insert("sf".to_owned(), dep);

    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(
        servers[0].url.as_str(),
        "https://api.example.com/api/v1/mcp/sf/mcp",
        "an accessor-backed external server is proxied through the gateway, not its raw endpoint",
    );
}

#[test]
fn load_managed_mcp_servers_external_without_accessor_keeps_raw_url() {
    let mut config = ServicesConfig::default();
    let mut dep = make_deployment("direct", true, Some("https://direct.example.com/mcp"));
    dep.server_type = McpServerType::External;
    config.mcp_servers.insert("direct".to_owned(), dep);

    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(
        servers[0].url.as_str(),
        "https://direct.example.com/mcp",
        "an external server with no accessor keeps its raw endpoint (client reaches it directly)",
    );
}

#[test]
fn load_plugins_empty_config_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let plugins_root = dir.path().join("plugins");
    let config = ServicesConfig::default();
    let no_disabled = std::collections::BTreeSet::new();
    let content = BundleContent {
        skills: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &no_disabled,
        plugins_root: &plugins_root,
    };
    let plugins = load_plugins(&config, &content).expect("load plugins");
    assert!(plugins.is_empty());
}

#[test]
fn catalog_content_loads_once_and_exposes_borrowed_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let services = ServicesConfig::default();
    let catalog = CatalogContent::load(&services, dir.path(), "https://api.example.com")
        .expect("load catalog content");

    let content = catalog.as_content();
    assert!(content.skills.is_empty());
    assert!(content.agents.is_empty());
    assert!(content.mcp_servers.is_empty());
    assert!(content.disabled_mcp_servers.is_empty());
    assert!(content.plugins_root.ends_with("plugins"));
}

#[test]
fn load_cached_reuses_until_the_skills_tree_changes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("my_skill");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: my_skill\nname: My Skill\ndescription: a test skill\n",
    )
    .expect("write config");
    fs::write(skill_dir.join("index.md"), "first body").expect("write content");

    let services = ServicesConfig::default();
    let url = "https://api.example.com";

    let first = CatalogContent::load_cached(&services, dir.path(), url).expect("first load");
    let second = CatalogContent::load_cached(&services, dir.path(), url).expect("second load");
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "an unchanged skills tree must return the cached catalogue"
    );
    assert_eq!(first.as_content().skills.len(), 1);
    assert_eq!(first.as_content().skills[0].instructions, "first body");

    fs::write(skill_dir.join("index.md"), "a longer second body").expect("rewrite content");
    let third = CatalogContent::load_cached(&services, dir.path(), url).expect("third load");
    assert!(
        !std::sync::Arc::ptr_eq(&first, &third),
        "a changed skill file must invalidate the cache"
    );
    assert_eq!(
        third.as_content().skills[0].instructions,
        "a longer second body",
        "the rebuilt catalogue reflects the new on-disk content"
    );
}

#[test]
fn disabled_mcp_server_names_returns_only_disabled() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("on-mcp".to_owned(), make_deployment("on-mcp", true, None));
    config.mcp_servers.insert(
        "off-mcp".to_owned(),
        make_deployment("off-mcp", false, None),
    );

    let disabled = disabled_mcp_server_names(&config);
    assert!(disabled.contains("off-mcp"), "disabled server is reported");
    assert!(
        !disabled.contains("on-mcp"),
        "an enabled server is not reported as disabled",
    );
    assert_eq!(disabled.len(), 1, "only disabled servers are reported");
}

fn write_artifact(root: &std::path::Path, id: &str, config: &str, content: Option<&str>) {
    let dir = root.join("artifacts").join(id);
    fs::create_dir_all(&dir).expect("create artifact dir");
    fs::write(dir.join("config.yaml"), config).expect("write config");
    if let Some(html) = content {
        fs::write(dir.join("content.html"), html).expect("write content");
    }
}

#[test]
fn load_artifacts_no_dir_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let result = load_artifacts(dir.path()).expect("no error when artifacts dir absent");
    assert!(result.is_empty());
}

#[test]
fn load_artifacts_reads_valid_artifact_with_default_content_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_artifact(
        dir.path(),
        "pipeline",
        "id: pipeline\nname: Pipeline\ndescription: opps\nversion: \"3\"\nplugin_id: sfdc\nstarred: true\nmcp_tools:\n  - mcp__salesforce__query_opportunities\n",
        Some("<table><tr><td>data</td></tr></table>"),
    );

    let artifacts = load_artifacts(dir.path()).expect("load artifacts");
    assert_eq!(artifacts.len(), 1);
    let a = &artifacts[0];
    assert_eq!(a.id.as_str(), "pipeline");
    assert_eq!(a.plugin_id.as_str(), "sfdc");
    assert_eq!(a.version, "3");
    assert!(a.starred);
    assert_eq!(a.mcp_tools, vec!["mcp__salesforce__query_opportunities".to_owned()]);
    assert!(a.content.contains("<table>"));
    assert_eq!(a.sha256.as_str().len(), 64, "digest is 64 hex chars");
}

#[test]
fn load_artifacts_drops_artifact_with_missing_content() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_artifact(
        dir.path(),
        "empty",
        "id: empty\nname: Empty\ndescription: x\nplugin_id: sfdc\nmcp_tools:\n  - mcp__x__y\n",
        None,
    );
    let artifacts = load_artifacts(dir.path()).expect("load artifacts");
    assert!(artifacts.is_empty(), "artifact with no HTML content is dropped");
}

#[test]
fn load_artifacts_drops_artifact_with_no_mcp_tools() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_artifact(
        dir.path(),
        "toolless",
        "id: toolless\nname: Toolless\ndescription: x\nplugin_id: sfdc\n",
        Some("<table></table>"),
    );
    let artifacts = load_artifacts(dir.path()).expect("load artifacts");
    assert!(artifacts.is_empty(), "artifact with no mcp_tools is dropped");
}

#[test]
fn load_artifacts_skips_disabled() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_artifact(
        dir.path(),
        "off",
        "id: off\nname: Off\ndescription: x\nplugin_id: sfdc\nenabled: false\nmcp_tools:\n  - mcp__x__y\n",
        Some("<table></table>"),
    );
    let artifacts = load_artifacts(dir.path()).expect("load artifacts");
    assert!(artifacts.is_empty(), "disabled artifact is not loaded");
}
