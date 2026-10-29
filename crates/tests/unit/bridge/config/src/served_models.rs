use systemprompt_bridge::config::gateways::{
    GatewayProfileId, model_union, models_served_elsewhere,
};
use systemprompt_bridge::proxy::forward::{ForwardError, refuse_foreign_model};

fn id(s: &str) -> GatewayProfileId {
    GatewayProfileId::try_new(s).unwrap()
}

fn profiles() -> Vec<(GatewayProfileId, Vec<String>)> {
    vec![
        (id("acme"), vec!["claude-sonnet-5".into(), "gpt-5".into()]),
        (
            id("client-b"),
            vec!["claude-sonnet-5".into(), "gemini-3".into()],
        ),
    ]
}

#[test]
fn the_machine_model_list_is_the_union_of_every_profile() {
    assert_eq!(
        model_union(&profiles()),
        ["claude-sonnet-5", "gemini-3", "gpt-5"]
    );
}

#[test]
fn only_models_the_active_gateway_does_not_serve_are_another_profiles() {
    let elsewhere = models_served_elsewhere(&id("acme"), &profiles());
    assert_eq!(elsewhere.len(), 1);
    assert_eq!(elsewhere["gemini-3"], id("client-b"));
}

#[test]
fn the_proxy_refuses_another_gateways_model_with_a_switch_hint() {
    let elsewhere = models_served_elsewhere(&id("acme"), &profiles());
    let err = refuse_foreign_model(&elsewhere, br#"{"model":"gemini-3","messages":[]}"#)
        .expect_err("another gateway's model is refused");
    assert!(matches!(err, ForwardError::ModelServedElsewhere { .. }));
    assert_eq!(err.status().as_u16(), 400);
    assert!(
        err.client_detail().contains("client-b"),
        "{}",
        err.client_detail()
    );

    refuse_foreign_model(&elsewhere, br#"{"model":"claude-sonnet-5"}"#).expect("served here");
    refuse_foreign_model(&elsewhere, br#"{"model":"unknown-to-all"}"#)
        .expect("a model no profile lists is the gateway's to judge");
    refuse_foreign_model(&elsewhere, b"not json").expect("non-JSON bodies pass through");
}
