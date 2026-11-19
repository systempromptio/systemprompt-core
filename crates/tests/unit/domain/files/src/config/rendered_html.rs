//! `files.documents.renderedHtml`: origin entries that are exactly an
//! `https` origin and nothing else, an `agentAuthored` tier refused at load
//! when it is wider than the top level, every switch off by default, and
//! each tier's CSP built once with only its own origins.

use std::path::Path;

use systemprompt_files::{
    DocumentsConfig, FilesConfig, RemoteOrigin, RenderedHtmlConfig, RenderedHtmlTier,
};

fn documents(yaml: &str) -> Result<DocumentsConfig, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

fn origins(list: &[&str]) -> Vec<RemoteOrigin> {
    list.iter()
        .map(|origin| RemoteOrigin::try_new(*origin).expect("origin"))
        .collect()
}

#[test]
fn every_switch_is_off_and_every_list_empty_by_default() {
    for config in [
        DocumentsConfig::default().rendered_html,
        documents("enabled: true\n").expect("parse").rendered_html,
        documents("renderedHtml: {}\n")
            .expect("parse")
            .rendered_html,
    ] {
        assert_eq!(config, RenderedHtmlConfig::default());
        for tier in [config.top(), config.agent_authored()] {
            assert!(!tier.scripts);
            assert!(tier.remote_origins.is_empty());
        }
    }
}

#[test]
fn an_https_origin_written_as_its_own_origin_is_accepted() {
    for origin in [
        "https://cdn.example.com",
        "https://cdn.example.com:8443",
        "https://fonts.gstatic.com",
        "https://203.0.113.9",
        "https://xn--bcher-kva.de",
    ] {
        let parsed = RemoteOrigin::try_new(origin).expect(origin);
        assert_eq!(parsed.as_str(), origin);
    }
}

#[test]
fn anything_but_exactly_an_https_origin_is_refused() {
    for origin in [
        "",
        "cdn.example.com",
        "http://cdn.example.com",
        "wss://cdn.example.com",
        "data:",
        "https:",
        "https://cdn.example.com/",
        "https://cdn.example.com/lib.js",
        "https://cdn.example.com?x=1",
        "https://cdn.example.com#top",
        "https://user@cdn.example.com",
        "https://user:secret@cdn.example.com",
        "https://*.example.com",
        "*",
        "https://CDN.example.com",
        "https://cdn.example.com:443",
        "https://bücher.de",
        "https://[::1]",
        "https://cdn.example.com;script-src",
        "https://cdn.example.com 'unsafe-inline'",
        "https://cdn.example.com'",
        "https://cdn.example.com\"",
        "https://cdn.example.com,evil.example",
        "https://cdn.example.com\n",
        " https://cdn.example.com",
    ] {
        assert!(
            RemoteOrigin::try_new(origin).is_err(),
            "{origin:?} was accepted"
        );
    }
}

#[test]
fn a_bad_origin_in_the_profile_is_refused_at_load() {
    let refused =
        documents("renderedHtml:\n  remoteOrigins: ['https://cdn.example.com; script-src *']\n");
    assert!(refused.is_err());
    let yaml = "files:\n  documents:\n    renderedHtml:\n      remoteOrigins: ['https://cdn.example.com/x']\n";
    assert!(FilesConfig::parse_yaml(yaml, Path::new("files.yaml"), &|_| None).is_err());
}

#[test]
fn the_removed_remote_assets_switch_is_refused_at_load() {
    assert!(documents("renderedHtml:\n  remoteAssets: true\n").is_err());
}

#[test]
fn an_agent_authored_tier_wider_than_the_top_level_is_refused_at_load() {
    for yaml in [
        "renderedHtml:\n  agentAuthored:\n    scripts: true\n",
        "renderedHtml:\n  scripts: true\n  agentAuthored:\n    remoteOrigins: ['https://cdn.example.com']\n",
        "renderedHtml:\n  remoteOrigins: ['https://a.example.com']\n  agentAuthored:\n    remoteOrigins: ['https://b.example.com']\n",
    ] {
        assert!(documents(yaml).is_err(), "{yaml}");
    }
    let top = RenderedHtmlTier {
        scripts: false,
        remote_origins: Vec::new(),
    };
    let wider = RenderedHtmlTier {
        scripts: true,
        remote_origins: Vec::new(),
    };
    assert!(RenderedHtmlConfig::new(top, wider).is_err());
}

#[test]
fn a_narrower_agent_authored_tier_is_kept_and_chosen_for_agent_content() {
    let config = documents(
        "renderedHtml:\n  scripts: true\n  remoteOrigins: ['https://a.example.com', 'https://b.example.com']\n  agentAuthored:\n    remoteOrigins: ['https://b.example.com']\n",
    )
    .expect("parse")
    .rendered_html;
    assert!(config.tier(false).scripts);
    assert_eq!(
        config.tier(false).remote_origins,
        origins(&["https://a.example.com", "https://b.example.com"])
    );
    assert!(!config.tier(true).scripts);
    assert_eq!(
        config.tier(true).remote_origins,
        origins(&["https://b.example.com"])
    );
    let round_trip: RenderedHtmlConfig =
        serde_yaml::from_str(&serde_yaml::to_string(&config).expect("serialise")).expect("parse");
    assert_eq!(round_trip, config);
}

const LOCKED: &str = "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; \
                      font-src data:; media-src data: blob:; form-action 'none'; base-uri 'none'";

fn csp(config: &RenderedHtmlConfig, agent_content: bool) -> String {
    config
        .csp(agent_content)
        .to_str()
        .expect("ascii policy")
        .to_owned()
}

#[test]
fn the_default_policy_runs_nothing_and_loads_only_what_the_page_embeds() {
    let built = RenderedHtmlConfig::new(RenderedHtmlTier::default(), RenderedHtmlTier::default())
        .expect("default tiers");
    for config in [RenderedHtmlConfig::default(), built] {
        assert_eq!(csp(&config, false), LOCKED);
        assert_eq!(csp(&config, true), LOCKED);
    }
}

#[test]
fn each_tier_s_policy_lists_only_its_own_origins() {
    let config = documents(
        "renderedHtml:\n  scripts: true\n  remoteOrigins: ['https://a.example.com', 'https://b.example.com:8443']\n  agentAuthored:\n    remoteOrigins: ['https://b.example.com:8443']\n",
    )
    .expect("parse")
    .rendered_html;
    assert_eq!(
        csp(&config, false),
        "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' \
         https://a.example.com https://b.example.com:8443; style-src 'unsafe-inline' \
         https://a.example.com https://b.example.com:8443; img-src data: blob: \
         https://a.example.com https://b.example.com:8443; font-src data: https://a.example.com \
         https://b.example.com:8443; media-src data: blob: https://a.example.com \
         https://b.example.com:8443; form-action 'none'; base-uri 'none'"
    );
    assert_eq!(
        csp(&config, true),
        "sandbox; default-src 'none'; style-src 'unsafe-inline' https://b.example.com:8443; \
         img-src data: blob: https://b.example.com:8443; font-src data: \
         https://b.example.com:8443; media-src data: blob: https://b.example.com:8443; \
         form-action 'none'; base-uri 'none'"
    );
}
