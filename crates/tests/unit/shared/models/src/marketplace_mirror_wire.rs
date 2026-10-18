use serde_json::json;
use systemprompt_manifest::services::{
    ExternalMarketplace, ExternalMarketplaceSource, ExternalPluginEntry, ExternalPluginSource,
};
use systemprompt_models::bridge::manifest::{ManifestExternalMarketplace, ManifestExternalPlugin};

fn wire<T: serde::Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).expect("serialise")
}

#[test]
fn marketplace_sources_flatten_to_the_claude_code_wire_shape() {
    let github: ManifestExternalMarketplace = ExternalMarketplace {
        name: "acme".to_owned(),
        source: ExternalMarketplaceSource::Github {
            repo: "acme/plugins".to_owned(),
            reference: Some("v1".to_owned()),
        },
    }
    .into();
    assert_eq!(
        wire(github),
        json!({"name": "acme", "source": {"source": "github", "repo": "acme/plugins", "ref": "v1"}})
    );

    let git: ManifestExternalMarketplace = ExternalMarketplace {
        name: "mirror".to_owned(),
        source: ExternalMarketplaceSource::Git {
            url: "https://git.example.com/m.git".to_owned(),
            reference: None,
        },
    }
    .into();
    assert_eq!(
        wire(git),
        json!({"name": "mirror", "source": {"source": "git", "url": "https://git.example.com/m.git"}})
    );
}

fn entry(source: ExternalPluginSource) -> ManifestExternalPlugin {
    ExternalPluginEntry {
        name: "tool".to_owned(),
        source,
        description: Some("d".to_owned()),
        version: Some("1.2.3".to_owned()),
        strict: Some(false),
        skills: None,
    }
    .into()
}

#[test]
fn plugin_sources_keep_their_pin_and_location() {
    let sha = "0123456789abcdef0123456789abcdef01234567".to_owned();
    let github = wire(entry(ExternalPluginSource::Github {
        repo: "acme/tool".to_owned(),
        reference: None,
        sha: sha.clone(),
    }));
    assert_eq!(github["source"]["source"], "github");
    assert_eq!(github["source"]["repo"], "acme/tool");
    assert_eq!(github["source"]["sha"], sha.as_str());
    assert!(github["source"].get("url").is_none());
    assert_eq!(github["name"], "tool");
    assert_eq!(github["version"], "1.2.3");
    assert_eq!(github["strict"], false);

    let url = wire(entry(ExternalPluginSource::Url {
        url: "https://git.example.com/t.git".to_owned(),
        reference: Some("main".to_owned()),
        sha: sha.clone(),
    }));
    assert_eq!(url["source"]["source"], "url");
    assert_eq!(url["source"]["url"], "https://git.example.com/t.git");
    assert_eq!(url["source"]["ref"], "main");
    assert!(url["source"].get("repo").is_none());

    let subdir = wire(entry(ExternalPluginSource::GitSubdir {
        url: "https://git.example.com/mono.git".to_owned(),
        path: "plugins/tool".to_owned(),
        reference: None,
        sha,
    }));
    assert_eq!(subdir["source"]["source"], "git-subdir");
    assert_eq!(subdir["source"]["path"], "plugins/tool");
    assert_eq!(subdir["source"]["url"], "https://git.example.com/mono.git");
}
