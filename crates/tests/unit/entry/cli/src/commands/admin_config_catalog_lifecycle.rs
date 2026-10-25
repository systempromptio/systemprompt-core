use systemprompt_cli::admin::config::catalog::{self, CatalogCommands};
use systemprompt_cli::admin::config::credential::{self, CredentialAddArgs, CredentialCommands};
use systemprompt_cli::admin::config::endpoint::{self, EndpointAddArgs, EndpointCommands};
use systemprompt_cli::admin::config::model::{self, ModelAddArgs, ModelCommands};
use systemprompt_cli::admin::config::provider::{self, ProviderAddArgs, ProviderCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::ModelId;

const HELPER: &str = "commands::admin_config_catalog_lifecycle::catalog_file_lifecycle_helper";

const BOOT_CATALOG: &str = r#"
credentials:
  boot_provider_key: { kind: platform, secret: boot_provider_key }
  bad_vertex: { kind: platform, secret: bad_vertex }
providers:
  boot-provider:
    credential: boot_provider_key
    endpoints:
      api: { wire: openai-chat, url: "https://boot.example.invalid/v1" }
  vertex:
    credential: bad_vertex
    endpoints:
      gemini:
        wire: gemini
        url: "https://us-central1-aiplatform.googleapis.com/v1/projects/{project}/locations/us-central1/publishers/google"
"#;

fn json_config(boot: &systemprompt_test_fixtures::TestBootstrap) -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_boot(Some(boot.boot.clone()))
}

fn yaml(path: &std::path::Path) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(path).expect("read catalog file"))
        .expect("catalog file is YAML")
}

fn files(ai: &std::path::Path) -> Vec<String> {
    [
        "credentials.yaml",
        "providers.yaml",
        "models.yaml",
        "deployments.yaml",
    ]
    .iter()
    .map(|name| std::fs::read_to_string(ai.join(name)).unwrap_or_default())
    .collect()
}

fn provider_add(id: &str, credential: &str, headers: Vec<String>) -> ProviderCommands {
    ProviderCommands::Add(ProviderAddArgs {
        id: id.to_owned(),
        credential: credential.to_owned(),
        display_name: None,
        user_agent: None,
        headers,
        endpoint: None,
        wire: Some("openai-chat".to_owned()),
        url: Some("https://models.example.invalid/v1".to_owned()),
    })
}

fn endpoint_add(wire: &str, url: &str) -> EndpointCommands {
    EndpointCommands::Add(EndpointAddArgs {
        provider: "fixture-openai".to_owned(),
        id: "api".to_owned(),
        wire: wire.to_owned(),
        url: url.to_owned(),
    })
}

fn model_add(deployment: &str) -> ModelCommands {
    ModelCommands::Add(ModelAddArgs {
        id: ModelId::new("fixture-model"),
        deployments: vec![deployment.to_owned()],
        upstream_model: Some("vendor-model-2026".to_owned()),
        display_name: None,
        clients: Some(vec!["openai".to_owned()]),
    })
}

#[tokio::test]
#[ignore = "re-executed by catalog_file_lifecycle_is_durable_and_structured"]
async fn catalog_file_lifecycle_helper() {
    unsafe {
        std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", "bad_vertex");
        std::env::set_var("bad_vertex", r#"{"type":"service_account"}"#);
    }
    let boot = systemprompt_test_fixtures::init_services_bootstrap(BOOT_CATALOG);
    let ai = boot.services_path.join("ai");
    let providers_path = ai.join("providers.yaml");
    let config = json_config(&boot);

    println!("BEGIN_PROVIDER_LIST");
    provider::execute(&ProviderCommands::List, &config)
        .await
        .expect("list booted provider");
    println!("END_PROVIDER_LIST");

    let original = std::fs::read_to_string(&providers_path).expect("booted providers.yaml");
    std::fs::write(&providers_path, "providers: [\n").expect("write broken providers.yaml");
    let malformed = provider::execute(
        &provider_add("fixture-openai", "boot_provider_key", vec![]),
        &config,
    )
    .await
    .expect_err("malformed catalog source is visible");
    println!("MALFORMED_ERROR={malformed:#}");
    assert_eq!(
        std::fs::read_to_string(&providers_path).unwrap(),
        "providers: [\n"
    );
    std::fs::write(&providers_path, original).expect("repair providers.yaml");

    credential::execute(
        &CredentialCommands::Add(CredentialAddArgs {
            id: "fixture_openai_key".to_owned(),
            kind: "platform".to_owned(),
            secret: Some("fixture_openai_key".to_owned()),
            label: None,
        }),
        &config,
    )
    .await
    .expect("add credential");
    let before_invalid = files(&ai);
    for result in [
        provider::execute(
            &provider_add(
                "bad-header",
                "fixture_openai_key",
                vec!["missing-separator".to_owned()],
            ),
            &config,
        )
        .await,
        provider::execute(
            &provider_add("bad-credential", "absent_key", vec![]),
            &config,
        )
        .await,
        endpoint::execute(
            &endpoint_add("unknown-wire", "https://invalid.example"),
            &config,
        )
        .await,
    ] {
        assert!(result.is_err());
        assert_eq!(files(&ai), before_invalid);
    }

    println!("BEGIN_PROVIDER_ADD");
    provider::execute(
        &provider_add(
            "fixture-openai",
            "fixture_openai_key",
            vec!["X-Fixture=enabled".to_owned()],
        ),
        &config,
    )
    .await
    .expect("add provider with its first endpoint");
    println!("END_PROVIDER_ADD");
    let stored = yaml(&providers_path);
    assert_eq!(
        stored["providers"]["fixture-openai"]["extra_headers"]["X-Fixture"],
        "enabled"
    );
    assert_eq!(
        stored["providers"]["fixture-openai"]["endpoints"]["api"]["wire"],
        "openai-chat"
    );
    let root = std::fs::read_to_string(boot.services_path.join("config/config.yaml")).unwrap();
    assert!(
        !root.contains("../ai/providers.yaml"),
        "the catalog is read by convention: {root}"
    );

    println!("BEGIN_MODEL_ADD");
    model::execute(&model_add("fixture-openai/api"), &config)
        .await
        .expect("add model");
    println!("END_MODEL_ADD");
    let models = yaml(&ai.join("models.yaml"));
    assert_eq!(
        models["models"]["fixture-model"]["upstream_model"],
        "vendor-model-2026"
    );
    assert_eq!(models["models"]["fixture-model"]["clients"][0], "openai");
    let deployments = yaml(&ai.join("deployments.yaml"));
    assert_eq!(
        deployments["deployments"]["fixture-model"][0],
        "fixture-openai/api"
    );

    endpoint::execute(
        &endpoint_add("openai-responses", "https://responses.example.invalid/v1"),
        &config,
    )
    .await
    .expect("replace endpoint connectivity");
    let stored = yaml(&providers_path);
    assert_eq!(
        stored["providers"]["fixture-openai"]["endpoints"]["api"]["wire"],
        "openai-responses"
    );
    assert_eq!(
        yaml(&ai.join("models.yaml"))["models"]["fixture-model"]["upstream_model"],
        "vendor-model-2026"
    );

    let before_unknown = files(&ai);
    let unknown = model::execute(&model_add("absent-provider/api"), &config)
        .await
        .expect_err("a model cannot be deployed on an absent endpoint");
    println!("UNKNOWN_PROVIDER_ERROR={unknown:#}");
    assert_eq!(files(&ai), before_unknown);
    let missing_model = model::execute(
        &ModelCommands::Remove {
            id: ModelId::new("absent-model"),
        },
        &config,
    )
    .await
    .expect_err("removing an absent model is visible");
    println!("MISSING_MODEL_ERROR={missing_model:#}");
    assert_eq!(files(&ai), before_unknown);

    println!("BEGIN_MODEL_REMOVE");
    model::execute(
        &ModelCommands::Remove {
            id: ModelId::new("fixture-model"),
        },
        &config,
    )
    .await
    .expect("remove model");
    println!("END_MODEL_REMOVE");
    assert!(
        yaml(&ai.join("models.yaml"))["models"]
            .get("fixture-model")
            .is_none()
    );

    println!("BEGIN_PROVIDER_REMOVE");
    provider::execute(
        &ProviderCommands::Remove {
            id: "fixture-openai".to_owned(),
        },
        &config,
    )
    .await
    .expect("remove provider");
    println!("END_PROVIDER_REMOVE");
    let after_remove = files(&ai);
    let missing_provider = provider::execute(
        &ProviderCommands::Remove {
            id: "fixture-openai".to_owned(),
        },
        &config,
    )
    .await
    .expect_err("removing an absent provider is visible");
    println!("MISSING_PROVIDER_ERROR={missing_provider:#}");
    assert_eq!(files(&ai), after_remove);

    println!("BEGIN_DISCOVERY_EMPTY");
    catalog::execute(&CatalogCommands::Discovery, &config.cli, None)
        .expect("missing discovery report is informative");
    println!("END_DISCOVERY_EMPTY");

    use systemprompt_scheduler::jobs::vertex_discovery::VertexDiscoveryJob;
    let report = VertexDiscoveryJob::probe(
        &systemprompt_loader::ServicesState::load(&boot.boot).expect("services state"),
        boot.boot.secrets().get(),
    )
    .await
    .expect("discovery job produces a report");
    println!("DISCOVERY_RAN_AT={}", report.ran_at);
    assert_eq!(report.failed_publishers.len(), 1, "{report:?}");
    assert!(
        report.failed_publishers[0].starts_with("vertex: service-account key is malformed:"),
        "{report:?}"
    );
    println!("BEGIN_DISCOVERY_PARTIAL");
    systemprompt_cli::admin::config::catalog_discovery::render_discovery(
        Some(&report),
        &config.cli,
    )
    .expect("partial discovery report is rendered");
    println!("END_DISCOVERY_PARTIAL");
}

fn marked_json(stdout: &str, name: &str) -> serde_json::Value {
    let begin = format!("BEGIN_{name}");
    let end = format!("END_{name}");
    let text = stdout
        .split_once(&begin)
        .and_then(|(_, tail)| tail.split_once(&end))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing {begin}/{end}: {stdout}"));
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{name}: {error}: {text}"))
}

fn marked_json_stream(stdout: &str, name: &str) -> Vec<serde_json::Value> {
    let begin = format!("BEGIN_{name}");
    let end = format!("END_{name}");
    let text = stdout
        .split_once(&begin)
        .and_then(|(_, tail)| tail.split_once(&end))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing {begin}/{end}: {stdout}"));
    serde_json::Deserializer::from_str(text)
        .into_iter::<serde_json::Value>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("{name}: {error}: {text}"))
}

#[test]
fn catalog_file_lifecycle_is_durable_and_structured() {
    let output = std::process::Command::new(std::env::current_exe().expect("unit-test binary"))
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .output()
        .expect("re-execute catalog helper");
    assert!(
        output.status.success(),
        "catalog helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("catalog output UTF-8");
    assert!(stdout.contains("MALFORMED_ERROR="), "{stdout}");
    assert!(stdout.contains("providers.yaml"), "{stdout}");
    assert!(stdout.contains("UNKNOWN_PROVIDER_ERROR="), "{stdout}");
    assert!(stdout.contains("absent-provider/api"), "{stdout}");
    assert!(stdout.contains("MISSING_MODEL_ERROR="), "{stdout}");
    assert!(stdout.contains("absent-model"), "{stdout}");
    assert!(stdout.contains("MISSING_PROVIDER_ERROR="), "{stdout}");

    for (marker, title, expected) in [
        (
            "PROVIDER_ADD",
            "Provider Updated",
            "Provider fixture-openai saved",
        ),
        ("MODEL_ADD", "Model Updated", "Model fixture-model saved"),
        (
            "MODEL_REMOVE",
            "Model Removed",
            "Model fixture-model removed",
        ),
        (
            "PROVIDER_REMOVE",
            "Provider Removed",
            "Provider fixture-openai removed",
        ),
    ] {
        let value = marked_json(&stdout, marker);
        assert_eq!(value["title"], title, "{value}");
        let message = value["sections"]
            .as_array()
            .expect("card sections")
            .iter()
            .find(|section| section["heading"] == "message")
            .expect("message section")["content"]
            .as_str()
            .expect("message text");
        assert!(message.starts_with(expected), "{value}");
    }

    let list = marked_json(&stdout, "PROVIDER_LIST");
    let item = list["items"]
        .as_array()
        .expect("provider list items")
        .iter()
        .find(|item| {
            item["title"]
                .as_str()
                .is_some_and(|title| title.contains("boot-provider"))
        })
        .unwrap_or_else(|| panic!("boot provider missing: {list}"));
    assert!(
        item["title"]
            .as_str()
            .unwrap()
            .contains("credential boot_provider_key"),
        "{list}"
    );

    let discovery = marked_json(&stdout, "DISCOVERY_EMPTY");
    assert!(
        discovery["messages"]
            .as_array()
            .expect("discovery messages")
            .iter()
            .any(|line| line["text"]
                .as_str()
                .is_some_and(|text| text.contains("has not run"))),
        "{discovery}"
    );

    let ran_at = stdout
        .lines()
        .find_map(|line| line.strip_prefix("DISCOVERY_RAN_AT="))
        .expect("captured discovery timestamp");
    let partial = marked_json_stream(&stdout, "DISCOVERY_PARTIAL");
    assert_eq!(partial.len(), 2, "{partial:?}");
    assert_eq!(partial[0]["artifact_type"], "table");
    let column_names = partial[0]["columns"]
        .as_array()
        .expect("discovery table columns")
        .iter()
        .map(|column| column["name"].as_str().expect("column name"))
        .collect::<Vec<_>>();
    assert_eq!(column_names, ["upstream_or_id", "state", "retires_on"]);
    assert_eq!(partial[0]["items"], serde_json::json!([]));
    let notes = partial[1]["messages"]
        .as_array()
        .expect("discovery report notes");
    assert!(
        notes.iter().any(|line| {
            line["level"] == "warning"
                && line["text"].as_str().is_some_and(|text| {
                    text.contains("vertex") && text.contains("the listing is partial")
                })
        }),
        "{partial:?}"
    );
    let expected_ran_at = format!("ran_at: {ran_at}");
    assert!(
        notes.iter().any(|line| {
            line["level"] == "info" && line["text"].as_str() == Some(expected_ran_at.as_str())
        }),
        "{partial:?}"
    );
}
