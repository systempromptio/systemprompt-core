//! The 0.72 `wire` spellings migrate to the canonical tags; the loader reads
//! only the canonical ones.

use systemprompt_cli::commands::admin::config::migrate::plan;
use systemprompt_manifest::services::catalog::EndpointConfig;

use crate::support::from_files;

fn providers(wire: &str) -> String {
    format!(
        "providers:\n  - name: alpha\n    wire: {wire}\n    surface: openai\n    endpoint: https://api.example.com/v1\n    api_key_secret: alpha\n    models:\n      - id: alpha-1\n        pricing: {{ input_per_million: 1.0, output_per_million: 2.0 }}\n"
    )
}

fn migrated_wire_tag(wire: &str) -> String {
    let tree = from_files(&[("providers.yaml", &providers(wire))]);
    let migrated = plan(&tree.services)
        .expect("legacy wire spelling migrates")
        .expect("a legacy tree yields a plan");
    let wires: Vec<&str> = migrated
        .document
        .providers
        .iter()
        .flat_map(|(_, provider)| provider.endpoints.values())
        .map(|endpoint| endpoint.wire.as_tag())
        .collect();
    assert_eq!(wires.len(), 1, "one endpoint per legacy entry: {wires:?}");
    wires[0].to_owned()
}

#[test]
fn legacy_openai_spellings_migrate_to_openai_chat() {
    for spelling in ["openai", "openai_chat", "openai-chat"] {
        assert_eq!(migrated_wire_tag(spelling), "openai-chat", "{spelling}");
    }
}

#[test]
fn legacy_responses_spelling_migrates_to_openai_responses() {
    for spelling in ["openai_responses", "openai-responses"] {
        assert_eq!(
            migrated_wire_tag(spelling),
            "openai-responses",
            "{spelling}"
        );
    }
}

#[test]
fn catalog_endpoint_refuses_legacy_wire_spellings() {
    for spelling in ["openai", "openai_chat", "openai_responses"] {
        let yaml = format!("wire: {spelling}\nurl: https://api.example.com/v1\n");
        let parsed = serde_yaml::from_str::<EndpointConfig>(&yaml);
        assert!(parsed.is_err(), "{spelling} must not parse as a wire");
    }
}
