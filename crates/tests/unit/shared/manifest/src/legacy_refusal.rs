//! A pre-0.73 tree is refused with the `config migrate` hint and never
//! half-loads (credential.md §9.7).

use std::path::Path;

use systemprompt_manifest::services::AiConfig;
use systemprompt_manifest::services::catalog::{
    CatalogDocument, CatalogDocumentError, MIGRATE_HINT, detect_legacy_shape,
};

fn fixture_ai(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../common/fixtures/catalog")
        .join(name)
        .join("legacy/ai")
}

#[test]
fn every_committed_legacy_tree_is_refused_with_the_migrate_hint() {
    for fixture in ["core-default", "byok-gateway", "vertex-multiregion"] {
        let path = fixture_ai(fixture).join("providers.yaml");
        let text = std::fs::read_to_string(&path).expect("legacy fixture present");
        let error = CatalogDocument::from_yaml_str("providers.yaml", &text)
            .expect_err("a 0.72 tree must not load");
        assert!(
            matches!(error, CatalogDocumentError::LegacyShape { .. }),
            "{fixture}: {error}"
        );
        assert!(
            error.to_string().contains(MIGRATE_HINT),
            "{fixture}: {error}"
        );
        let whole = systemprompt_catalog_fixtures::read_ai_dir(&fixture_ai(fixture))
            .expect_err("the directory is refused too");
        assert!(whole.contains(MIGRATE_HINT), "{fixture}: {whole}");
    }
}

#[test]
fn each_legacy_construct_is_named() {
    let cases = [
        (
            "providers:\n  - name: anthropic\n",
            "`providers:` as a list",
        ),
        (
            "ai:\n  default_provider: anthropic\n",
            "`ai.default_provider`",
        ),
        ("ai:\n  providers: {}\n", "`ai.providers`"),
        (
            "gateway:\n  default_provider: anthropic\n",
            "`gateway.default_provider`",
        ),
        (
            "gateway:\n  routes:\n    - model_pattern: claude-*\n      provider: anthropic\n",
            "`model_pattern`",
        ),
    ];
    for (yaml, expected) in cases {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).expect("yaml");
        let found = detect_legacy_shape(&value).expect("legacy detected");
        assert!(found.contains(expected), "{yaml} named {found}");
    }
    let current: serde_yaml::Value =
        serde_yaml::from_str("providers:\n  anthropic: { credential: a, endpoints: {} }\n")
            .expect("yaml");
    assert_eq!(detect_legacy_shape(&current), None);
}

#[test]
fn the_ai_policy_block_refuses_the_retired_provider_keys() {
    let refused: Result<AiConfig, _> = serde_yaml::from_str("default_provider: anthropic\n");
    assert!(refused.is_err());
    let refused: Result<AiConfig, _> = serde_yaml::from_str("providers: {}\n");
    assert!(refused.is_err());
    let ok: AiConfig = serde_yaml::from_str("history: { retention_days: 7 }\n").expect("parses");
    assert_eq!(ok.history.retention_days, 7);
}

#[test]
fn merging_refuses_an_id_declared_twice() {
    let a =
        CatalogDocument::from_yaml_str("a", "credentials:\n  k: { kind: platform, secret: k }\n")
            .expect("parses");
    let b = a.clone();
    let mut merged = CatalogDocument::default();
    merged.merge(a).expect("first");
    assert!(matches!(
        merged.merge(b),
        Err(CatalogDocumentError::DuplicateId {
            section: "credentials",
            ..
        })
    ));
}
