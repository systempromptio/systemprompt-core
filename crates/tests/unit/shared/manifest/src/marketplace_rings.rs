use systemprompt_manifest::services::{CohortRing, ManagedMarketplaceConfig};
use systemprompt_models::errors::ServicesValidationError;

fn ring(dimension: &str, value: &str) -> CohortRing {
    CohortRing {
        dimension: dimension.to_owned(),
        value: value.to_owned(),
    }
}

fn config(rings: Vec<CohortRing>) -> ManagedMarketplaceConfig {
    ManagedMarketplaceConfig {
        rings,
        ..ManagedMarketplaceConfig::default()
    }
}

fn refusal(config: &ManagedMarketplaceConfig) -> String {
    match config.validate().expect_err("refused") {
        ServicesValidationError::InvalidField(message) => message,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn distinct_rings_validate_and_are_found() {
    let cfg = config(vec![ring("role", "beta"), ring("ext:department", "eng")]);
    cfg.validate().expect("valid");
    assert_eq!(cfg.ring("role", "beta"), Some(&ring("role", "beta")));
    assert!(cfg.ring("role", "eng").is_none());
}

#[test]
fn empty_rings_validate() {
    ManagedMarketplaceConfig::default()
        .validate()
        .expect("valid");
}

#[test]
fn blank_dimension_or_value_is_refused() {
    assert!(refusal(&config(vec![ring("  ", "beta")])).contains("non-empty"));
    assert!(refusal(&config(vec![ring("role", "")])).contains("non-empty"));
}

#[test]
fn user_dimension_is_refused() {
    assert!(refusal(&config(vec![ring("user", "alice")])).contains("'user'"));
}

#[test]
fn duplicate_ring_is_refused() {
    let message = refusal(&config(vec![ring("role", "beta"), ring("role", "beta")]));
    assert!(message.contains("role=beta"));
    assert!(message.contains("twice"));
}

#[test]
fn yaml_rejects_unknown_ring_fields() {
    let err = serde_yaml::from_str::<ManagedMarketplaceConfig>(
        "rings:\n  - dimension: role\n    value: beta\n    weight: 2\n",
    )
    .expect_err("unknown field");
    assert!(err.to_string().contains("weight"));
}
