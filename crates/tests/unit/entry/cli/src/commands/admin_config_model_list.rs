use systemprompt_cli::admin::config::model::{self, ModelAddArgs, ModelCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::ModelId;

const CATALOG: &str = r#"
credentials:
  list_provider_key: { kind: platform, secret: list_provider_key }
providers:
  list-provider:
    credential: list_provider_key
    endpoints:
      api: { wire: openai-chat, url: "https://list.example.invalid/v1" }
      backup: { wire: openai-chat, url: "https://backup.example.invalid/v1" }
"#;

fn context(
    boot: &systemprompt_test_fixtures::TestBootstrap,
    format: OutputFormat,
) -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(format),
        EnvOverrides::default(),
    )
    .with_boot(Some(boot.boot.clone()))
}

fn models_yaml(boot: &systemprompt_test_fixtures::TestBootstrap) -> serde_yaml::Value {
    let path = boot.services_path.join("ai/models.yaml");
    serde_yaml::from_str(&std::fs::read_to_string(path).expect("models.yaml")).expect("yaml")
}

#[tokio::test]
async fn list_renders_declared_models_with_their_failover_chain() {
    let boot = systemprompt_test_fixtures::init_services_bootstrap(CATALOG);
    let json = context(&boot, OutputFormat::Json);

    model::execute(
        &ModelCommands::Add(ModelAddArgs {
            id: ModelId::new("list-model"),
            deployments: vec![
                "list-provider/api".to_owned(),
                "list-provider/backup".to_owned(),
            ],
            upstream_model: None,
            display_name: Some("List Model".to_owned()),
            clients: Some(vec!["openai,claude".to_owned()]),
        }),
        &json,
    )
    .await
    .expect("add model");
    let models = models_yaml(&boot);
    assert_eq!(models["models"]["list-model"]["display_name"], "List Model");
    assert_eq!(
        models["models"]["list-model"]["clients"]
            .as_sequence()
            .map(Vec::len),
        Some(2)
    );

    for format in [OutputFormat::Json, OutputFormat::Table] {
        model::execute(&ModelCommands::List, &context(&boot, format))
            .await
            .expect("list renders");
    }
}

#[tokio::test]
async fn an_invalid_client_family_is_refused_before_writing() {
    let boot = systemprompt_test_fixtures::init_services_bootstrap(CATALOG);
    let err = model::execute(
        &ModelCommands::Add(ModelAddArgs {
            id: ModelId::new("bad-clients-model"),
            deployments: vec!["list-provider/api".to_owned()],
            upstream_model: None,
            display_name: None,
            clients: Some(vec!["netscape".to_owned()]),
        }),
        &context(&boot, OutputFormat::Json),
    )
    .await
    .expect_err("unknown client family");
    assert!(format!("{err:#}").contains("netscape"), "{err:#}");
    let path = boot.services_path.join("ai/models.yaml");
    if path.exists() {
        assert!(
            models_yaml(&boot)["models"]
                .get("bad-clients-model")
                .is_none()
        );
    }
}
