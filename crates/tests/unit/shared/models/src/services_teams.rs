use systemprompt_models::services::{TeamsAppConfig, TeamsEndpoints};

#[test]
fn teams_app_config_without_endpoints_deserialises_to_public_cloud_defaults() {
    let yaml = r"
tenant_id: 11111111-1111-1111-1111-111111111111
app_id: app-abc
app_password_ref: teams_app_password
default_agent: support
";
    let cfg: TeamsAppConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.endpoints, TeamsEndpoints::default());
    assert_eq!(
        cfg.endpoints.openid_config_url,
        "https://login.botframework.com/v1/.well-known/openidconfiguration"
    );
    assert_eq!(
        cfg.endpoints.token_url,
        "https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token"
    );
    cfg.validate("default").unwrap();
}

#[test]
fn teams_app_config_explicit_endpoints_round_trip() {
    let yaml = r"
tenant_id: 11111111-1111-1111-1111-111111111111
app_id: app-abc
app_password_ref: teams_app_password
default_agent: support
endpoints:
  openid_config_url: https://login.botframework.us/v1/.well-known/openidconfiguration
  token_url: https://login.microsoftonline.us/botframework.com/oauth2/v2.0/token
";
    let cfg: TeamsAppConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        cfg.endpoints.openid_config_url,
        "https://login.botframework.us/v1/.well-known/openidconfiguration"
    );
    assert_ne!(cfg.endpoints, TeamsEndpoints::default());

    let round_tripped: TeamsAppConfig =
        serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
    assert_eq!(round_tripped.endpoints, cfg.endpoints);
}
