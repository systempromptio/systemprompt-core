use systemprompt_marketplace::import::{MarketplaceJson, MarketplacePluginEntry};

fn entry(json: &str) -> MarketplacePluginEntry {
    serde_json::from_str(json).expect("plugin entry parses")
}

#[test]
fn a_bare_entry_needs_only_a_name() {
    let e = entry(r#"{"name":"alpha"}"#);

    assert_eq!(e.name, "alpha");
    assert!(e.author_name().is_none());
    assert!(e.author_email().is_none());
    assert!(e.local_path().is_none());
    assert!(!e.source_is_remote());
}

#[test]
fn a_string_author_is_a_name_without_an_email() {
    let e = entry(r#"{"name":"alpha","author":"Acme"}"#);

    assert_eq!(e.author_name().as_deref(), Some("Acme"));
    assert!(e.author_email().is_none());
}

#[test]
fn an_object_author_yields_both_halves() {
    let e = entry(r#"{"name":"alpha","author":{"name":"Acme","email":"a@b.c"}}"#);

    assert_eq!(e.author_name().as_deref(), Some("Acme"));
    assert_eq!(e.author_email().as_deref(), Some("a@b.c"));
}

#[test]
fn a_non_string_non_object_author_reads_as_absent() {
    let e = entry(r#"{"name":"alpha","author":42}"#);

    assert!(e.author_name().is_none());
    assert!(e.author_email().is_none());
}

#[test]
fn a_string_source_is_the_local_path() {
    let e = entry(r#"{"name":"alpha","source":"./plugins/alpha"}"#);

    assert_eq!(e.local_path(), Some("./plugins/alpha"));
    assert!(!e.source_is_remote());
}

#[test]
fn an_object_source_is_read_from_path_then_source() {
    let by_path = entry(r#"{"name":"alpha","source":{"path":"./a"}}"#);
    let by_source = entry(r#"{"name":"alpha","source":{"source":"./b"}}"#);

    assert_eq!(by_path.local_path(), Some("./a"));
    assert_eq!(by_source.local_path(), Some("./b"));
}

#[test]
fn a_git_source_is_remote_and_has_no_local_path() {
    let e = entry(r#"{"name":"alpha","source":{"type":"git","repo":"acme/alpha"}}"#);

    assert!(e.local_path().is_none());
    assert!(e.source_is_remote());
}

#[test]
fn a_non_string_non_object_source_is_remote() {
    let e = entry(r#"{"name":"alpha","source":["./a"]}"#);

    assert!(e.local_path().is_none());
    assert!(e.source_is_remote());
}

#[test]
fn a_manifest_defaults_every_key_but_the_name() {
    let manifest: MarketplaceJson = serde_json::from_str(r#"{"name":"acme"}"#).expect("parses");

    assert_eq!(manifest.name, "acme");
    assert!(manifest.owner.name.is_empty());
    assert!(manifest.owner.email.is_empty());
    assert!(manifest.metadata.version.is_empty());
    assert!(manifest.metadata.plugin_root.is_none());
    assert!(manifest.plugins.is_empty());
}

#[test]
fn plugin_root_is_accepted_in_both_spellings() {
    let camel: MarketplaceJson =
        serde_json::from_str(r#"{"name":"acme","metadata":{"pluginRoot":"./pkgs"}}"#)
            .expect("parses");
    let snake: MarketplaceJson =
        serde_json::from_str(r#"{"name":"acme","metadata":{"plugin_root":"./pkgs"}}"#)
            .expect("parses");

    assert_eq!(camel.metadata.plugin_root.as_deref(), Some("./pkgs"));
    assert_eq!(snake.metadata.plugin_root.as_deref(), Some("./pkgs"));
}
