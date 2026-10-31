use systemprompt_identifiers::MarketplaceId;
use systemprompt_manifest::services::{
    ExternalPluginEntry, ExternalPluginSource, MarketplaceConfig, MarketplaceVisibility,
    PluginAuthor,
};
use systemprompt_models::plugin::PluginComponentRef;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn marketplace(external: Vec<ExternalPluginEntry>, vendored: &[&str]) -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new("ext-plugins"),
        name: "Test".to_owned(),
        description: "Test marketplace".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "systemprompt.io".to_owned(),
            email: "support@systemprompt.io".to_owned(),
        },
        keywords: vec![],
        license: "MIT".to_owned(),
        visibility: MarketplaceVisibility::Public,
        plugins: PluginComponentRef {
            include: vendored.iter().map(|p| (*p).to_owned()).collect(),
            ..PluginComponentRef::default()
        },
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        access: Default::default(),
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
        external_plugins: external,
        claude_code: None,
        opencode: None,
    }
}

fn entry(name: &str, source: ExternalPluginSource) -> ExternalPluginEntry {
    ExternalPluginEntry {
        name: name.to_owned(),
        source,
        description: None,
        version: None,
        strict: Some(false),
        skills: None,
    }
}

fn github(repo: &str, reference: Option<&str>, sha: &str) -> ExternalPluginSource {
    ExternalPluginSource::Github {
        repo: repo.to_owned(),
        reference: reference.map(str::to_owned),
        sha: sha.to_owned(),
    }
}

fn url(url: &str) -> ExternalPluginSource {
    ExternalPluginSource::Url {
        url: url.to_owned(),
        reference: None,
        sha: SHA.to_owned(),
    }
}

fn subdir(url: &str, path: &str) -> ExternalPluginSource {
    ExternalPluginSource::GitSubdir {
        url: url.to_owned(),
        path: path.to_owned(),
        reference: Some("main".to_owned()),
        sha: SHA.to_owned(),
    }
}

fn refusal(external: Vec<ExternalPluginEntry>, vendored: &[&str]) -> String {
    marketplace(external, vendored)
        .validate("ext-plugins")
        .expect_err("refused")
        .to_string()
}

#[test]
fn accessors_read_every_variant() {
    let gh = github("acme/tool", Some("v1"), SHA);
    assert_eq!(gh.reference(), Some("v1"));
    assert_eq!(gh.sha(), SHA);
    assert_eq!(url("https://git.example.com/tool.git").reference(), None);
    let sub = subdir("acme/mono", "./plugins/tool/");
    assert_eq!(sub.reference(), Some("main"));
    assert_eq!(sub.sha(), SHA);
}

#[test]
fn well_formed_entries_of_every_kind_validate() {
    marketplace(
        vec![
            entry("gh-tool", github("acme/tool", Some("v1.2"), SHA)),
            entry("url-tool", url("https://git.example.com/tool.git")),
            entry("url-gh", url("acme/tool")),
            entry("sub-tool", subdir("acme/mono", "./plugins/tool/")),
        ],
        &["vendored-one"],
    )
    .validate("ext-plugins")
    .expect("valid");
}

#[test]
fn an_invalid_name_is_refused() {
    let message = refusal(vec![entry("bad name", github("acme/tool", None, SHA))], &[]);
    assert!(message.contains("external plugin 'bad name'"), "{message}");
    assert!(message.contains("letters, digits"), "{message}");
}

#[test]
fn a_malformed_github_repo_is_refused() {
    let message = refusal(
        vec![entry("tool", github("acme/tool/extra", None, SHA))],
        &[],
    );
    assert!(message.contains("owner/repository"), "{message}");
}

#[test]
fn a_private_address_url_is_refused() {
    let message = refusal(vec![entry("tool", url("https://10.0.0.1/tool.git"))], &[]);
    assert!(message.contains("not a usable public URL"), "{message}");
}

#[test]
fn a_plain_http_url_is_refused() {
    let message = refusal(vec![entry("tool", url("http://localhost/tool.git"))], &[]);
    assert!(message.contains("must use https"), "{message}");
}

#[test]
fn an_escaping_subdir_path_is_refused() {
    let message = refusal(vec![entry("tool", subdir("acme/mono", "../outside"))], &[]);
    assert!(message.contains("relative path"), "{message}");
}

#[test]
fn a_bad_ref_is_refused() {
    let message = refusal(
        vec![entry(
            "tool",
            github("acme/tool", Some("feature branch"), SHA),
        )],
        &[],
    );
    assert!(message.contains("whitespace"), "{message}");
}

#[test]
fn an_unpinned_sha_is_refused() {
    for sha in ["main", "0123456789ABCDEF0123456789ABCDEF01234567", "abc123"] {
        let message = refusal(vec![entry("tool", github("acme/tool", None, sha))], &[]);
        assert!(
            message.contains("must be a full lowercase commit id"),
            "{message}"
        );
    }
}

#[test]
fn a_name_clashing_with_a_vendored_plugin_is_refused() {
    let message = refusal(
        vec![entry("shared", github("acme/tool", None, SHA))],
        &["shared"],
    );
    assert!(message.contains("vendors"), "{message}");
}

#[test]
fn a_duplicate_external_name_is_refused() {
    let message = refusal(
        vec![
            entry("tool", github("acme/tool", None, SHA)),
            entry("tool", url("https://git.example.com/tool.git")),
        ],
        &[],
    );
    assert!(message.contains("declared twice"), "{message}");
}
