use systemprompt_manifest::services::{GatewayConfigSpec, GatewayState};

fn spec(yaml: &str) -> GatewayConfigSpec {
    serde_yaml::from_str(yaml).expect("gateway spec parses")
}

#[test]
fn deserialization_always_yields_spec() {
    let state: GatewayState =
        serde_yaml::from_str("enabled: true\nauth_scheme: x-api-key\n").expect("parses");
    assert!(matches!(state, GatewayState::Spec(_)));
    assert!(state.resolved().is_none());
}

#[test]
fn deserialization_rejects_unknown_fields() {
    let err = serde_yaml::from_str::<GatewayState>("enabled: true\nbogus: 1\n")
        .expect_err("unknown field");
    assert!(err.to_string().contains("bogus"));
}

#[test]
fn spec_state_exposes_mutable_spec() {
    let mut state = GatewayState::Spec(spec("enabled: false\n"));
    state.as_spec_mut().expect("spec").enabled = true;
    assert!(state.into_spec().enabled);
}

#[test]
fn resolved_state_has_no_mutable_spec() {
    let mut state = GatewayState::Resolved(spec("enabled: true\n").resolve());
    assert!(state.as_spec_mut().is_none());
    assert!(state.resolved().expect("resolved").enabled);
}

#[test]
fn config_resolves_a_spec_on_demand() {
    let state = GatewayState::Spec(spec(
        "enabled: true\nauth_scheme: x-api-key\ninference_path_prefix: /llm\n",
    ));
    let config = state.config();
    assert!(config.enabled);
    assert_eq!(config.auth_scheme, "x-api-key");
    assert_eq!(config.inference_path_prefix, "/llm");
}

#[test]
fn resolved_round_trips_back_to_spec() {
    let resolved = spec("enabled: true\nauth_scheme: bearer\n").resolve();
    let state = GatewayState::Resolved(resolved);
    let back = state.clone().into_spec();
    assert!(back.enabled);
    assert_eq!(back.auth_scheme, "bearer");
    assert_eq!(state.config().auth_scheme, "bearer");
}

#[test]
fn serialization_matches_between_spec_and_resolved() {
    let source = spec("enabled: true\nauth_scheme: bearer\nautomatic_prompt_caching: false\n");
    let as_spec = serde_json::to_value(GatewayState::Spec(source.clone())).expect("spec");
    let as_resolved =
        serde_json::to_value(GatewayState::Resolved(source.resolve())).expect("resolved");
    assert_eq!(as_spec, as_resolved);
    assert_eq!(as_spec["enabled"], true);
    assert_eq!(as_spec["automatic_prompt_caching"], false);
}

#[test]
fn json_schema_is_the_spec_schema() {
    let state_schema = serde_json::to_value(schemars::schema_for!(GatewayState)).expect("schema");
    let spec_schema =
        serde_json::to_value(schemars::schema_for!(GatewayConfigSpec)).expect("schema");
    assert_eq!(state_schema, spec_schema);
    assert!(state_schema["properties"].get("routes").is_some());
}
