//! Tests for the services-file plumbing behind `admin config catalog` and
//! `admin config gateway`: splicing an include into the operator-authored root
//! aggregator, and the typed file shapes the setters load and write back.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::services_io::{GatewayFile, ProvidersFile, append_include};

fn root_with(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    std::fs::write(&root, body).unwrap();
    (dir, root)
}

#[test]
fn an_includes_key_on_the_very_first_line_is_extended_not_duplicated() {
    let (_dir, root) = root_with("includes:\n  - ../agents/a.yaml\nsettings:\n  x: 1\n");

    append_include(&root, "ai/providers.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(
        text,
        "includes:\n  - ai/providers.yaml\n  - ../agents/a.yaml\nsettings:\n  x: 1\n"
    );
    assert_eq!(
        text.matches("includes:").count(),
        1,
        "a second includes: key would shadow the first and drop every existing include"
    );
}

#[test]
fn a_root_that_does_not_exist_yet_is_created_with_the_include() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");

    append_include(&root, "ai/gateway.yaml").unwrap();

    assert_eq!(
        std::fs::read_to_string(&root).unwrap(),
        "includes:\n  - ai/gateway.yaml\n"
    );
}

#[test]
fn a_root_without_a_trailing_newline_keeps_its_last_line_intact() {
    let (_dir, root) = root_with("settings:\n  x: 1");

    append_include(&root, "ai/gateway.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(text, "settings:\n  x: 1\nincludes:\n  - ai/gateway.yaml\n");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
    assert_eq!(parsed["settings"]["x"], serde_yaml::Value::from(1));
}

#[test]
fn an_empty_includes_key_gains_its_first_entry() {
    let (_dir, root) = root_with("settings:\n  x: 1\nincludes:\n");

    append_include(&root, "ai/providers.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(
        text,
        "settings:\n  x: 1\nincludes:\n  - ai/providers.yaml\n"
    );
}

#[test]
fn a_quoted_existing_entry_counts_as_present() {
    for quoted in ["  - \"ai/providers.yaml\"\n", "  - 'ai/providers.yaml'\n"] {
        let (_dir, root) = root_with(&format!("includes:\n{quoted}"));

        append_include(&root, "ai/providers.yaml").unwrap();

        let text = std::fs::read_to_string(&root).unwrap();
        assert_eq!(
            text.matches("ai/providers.yaml").count(),
            1,
            "a quoted include must not be re-added unquoted: {text}"
        );
    }
}

#[test]
fn a_padded_existing_entry_is_matched_after_trimming() {
    let (_dir, root) = root_with("includes:\n  -    ai/gateway.yaml   \n");

    append_include(&root, "ai/gateway.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(text.matches("ai/gateway.yaml").count(), 1, "{text}");
}

#[test]
fn splicing_preserves_the_operator_comments_around_the_includes_list() {
    let (_dir, root) = root_with(
        "# root aggregator, hand written\nincludes:\n  # agents first\n  - ../agents/a.yaml\n\n# \
         tail note\nsettings:\n  x: 1\n",
    );

    append_include(&root, "ai/providers.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert!(text.contains("# root aggregator, hand written"), "{text}");
    assert!(text.contains("  # agents first"), "{text}");
    assert!(text.contains("# tail note"), "{text}");
}

#[test]
fn an_unmodelled_key_in_the_providers_file_is_rejected_rather_than_silently_dropped() {
    let err = serde_yaml::from_str::<ProvidersFile>("providers: []\nprovider: []\n").unwrap_err();
    assert!(
        err.to_string().contains("provider"),
        "a typo'd top-level key must fail the load, not round-trip away the operator's data: {err}"
    );

    let ok: ProvidersFile = serde_yaml::from_str("providers: []\n").unwrap();
    assert!(ok.providers.providers.is_empty());
}

#[test]
fn an_unmodelled_key_in_the_gateway_file_is_rejected_and_an_empty_file_defaults() {
    let err = serde_yaml::from_str::<GatewayFile>("gateways:\n  enabled: true\n").unwrap_err();
    assert!(err.to_string().contains("gateways"), "{err}");

    let empty: GatewayFile = serde_yaml::from_str("{}\n").unwrap();
    assert!(empty.gateway.is_none());
}
