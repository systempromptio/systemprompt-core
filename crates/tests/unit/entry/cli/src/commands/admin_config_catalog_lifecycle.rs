use systemprompt_cli::admin::config::catalog::{
    self, CatalogCommands, ModelAddArgs, ModelCommands, ProviderAddArgs, ProviderCommands,
};
use systemprompt_cli::{CliConfig, OutputFormat};

const HELPER: &str = "commands::admin_config_catalog_lifecycle::catalog_file_lifecycle_helper";

fn json_config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn providers(path: &std::path::Path) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(path).expect("read provider catalog"))
        .expect("provider catalog is YAML")
}

#[tokio::test]
#[ignore = "re-executed by catalog_file_lifecycle_is_durable_and_structured"]
async fn catalog_file_lifecycle_helper() {
    unsafe {
        std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", "bad_vertex");
        std::env::set_var("bad_vertex", r#"{"type":"service_account"}"#);
    }
    let boot = systemprompt_test_fixtures::init_services_bootstrap(
        r#"
providers:
  - name: boot-provider
    wire: openai-chat
    surface: openai
    endpoint: https://boot.example.invalid/v1
    api_key_secret: boot_provider_key
  - name: vertex
    wire: gemini
    surface: gemini
    endpoint: https://us-central1-aiplatform.googleapis.com/v1/projects/{project}/locations/us-central1/publishers/google
    api_key_secret: bad_vertex
"#,
    );
    let ai = boot.services_path.join("ai");
    std::fs::create_dir_all(&ai).expect("create AI services directory");
    let catalog_path = ai.join("providers.yaml");

    println!("BEGIN_PROVIDER_LIST");
    catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::List),
        &json_config(),
    )
    .await
    .expect("list booted provider");
    println!("END_PROVIDER_LIST");

    std::fs::write(&catalog_path, "providers: [\n").expect("write broken provider catalog");
    let malformed = catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::Add(ProviderAddArgs {
            name: "fixture-openai".to_owned(),
            wire: "openai-chat".to_owned(),
            surface: "openai".to_owned(),
            endpoint: "https://models.example.invalid/v1/chat/completions".to_owned(),
            api_key_secret: "fixture_openai_key".to_owned(),
            headers: vec![],
        })),
        &json_config(),
    )
    .await
    .expect_err("malformed provider source is visible");
    println!("MALFORMED_ERROR={malformed:#}");
    assert_eq!(
        std::fs::read_to_string(&catalog_path).unwrap(),
        "providers: [\n"
    );
    std::fs::remove_file(&catalog_path).expect("remove malformed provider source before repair");

    println!("BEGIN_PROVIDER_ADD");
    catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::Add(ProviderAddArgs {
            name: "fixture-openai".to_owned(),
            wire: "openai-chat".to_owned(),
            surface: "openai".to_owned(),
            endpoint: "https://models.example.invalid/v1/chat/completions".to_owned(),
            api_key_secret: "fixture_openai_key".to_owned(),
            headers: vec!["X-Fixture=enabled".to_owned()],
        })),
        &json_config(),
    )
    .await
    .expect("add provider");
    println!("END_PROVIDER_ADD");
    let stored = providers(&catalog_path);
    assert_eq!(stored["providers"][0]["name"], "fixture-openai");
    assert_eq!(
        stored["providers"][0]["extra_headers"]["X-Fixture"],
        "enabled"
    );
    let root = std::fs::read_to_string(boot.services_path.join("config/config.yaml"))
        .expect("read services root");
    assert!(root.contains("../ai/providers.yaml"), "{root}");

    let before_invalid = std::fs::read_to_string(&catalog_path).unwrap();
    for args in [
        ProviderAddArgs {
            name: "bad-wire".to_owned(),
            wire: "unknown-wire".to_owned(),
            surface: "openai".to_owned(),
            endpoint: "https://invalid.example".to_owned(),
            api_key_secret: "bad".to_owned(),
            headers: vec![],
        },
        ProviderAddArgs {
            name: "bad-surface".to_owned(),
            wire: "openai-chat".to_owned(),
            surface: "unknown-surface".to_owned(),
            endpoint: "https://invalid.example".to_owned(),
            api_key_secret: "bad".to_owned(),
            headers: vec![],
        },
        ProviderAddArgs {
            name: "bad-header".to_owned(),
            wire: "openai-chat".to_owned(),
            surface: "openai".to_owned(),
            endpoint: "https://invalid.example".to_owned(),
            api_key_secret: "bad".to_owned(),
            headers: vec!["missing-separator".to_owned()],
        },
    ] {
        assert!(
            catalog::execute(
                &CatalogCommands::Provider(ProviderCommands::Add(args)),
                &json_config(),
            )
            .await
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&catalog_path).unwrap(),
            before_invalid
        );
    }

    println!("BEGIN_MODEL_ADD");
    catalog::execute(
        &CatalogCommands::Model(ModelCommands::Add(ModelAddArgs {
            provider: "fixture-openai".to_owned(),
            id: "fixture-model".to_owned(),
            aliases: vec!["fixture-latest".to_owned()],
            upstream_model: Some("vendor-model-2026".to_owned()),
        })),
        &json_config(),
    )
    .await
    .expect("add model");
    println!("END_MODEL_ADD");
    let stored = providers(&catalog_path);
    let model = &stored["providers"][0]["models"][0];
    assert_eq!(model["id"], "fixture-model");
    assert_eq!(model["aliases"][0], "fixture-latest");
    assert_eq!(model["upstream_model"], "vendor-model-2026");

    catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::Add(ProviderAddArgs {
            name: "fixture-openai".to_owned(),
            wire: "openai-responses".to_owned(),
            surface: "openai".to_owned(),
            endpoint: "https://responses.example.invalid/v1/responses".to_owned(),
            api_key_secret: "fixture_responses_key".to_owned(),
            headers: vec![],
        })),
        &json_config(),
    )
    .await
    .expect("replace provider connectivity");
    let stored = providers(&catalog_path);
    assert_eq!(stored["providers"][0]["wire"], "openai-responses");
    assert_eq!(
        stored["providers"][0]["endpoint"],
        "https://responses.example.invalid/v1/responses"
    );
    assert_eq!(stored["providers"][0]["models"][0]["id"], "fixture-model");

    let before_unknown = std::fs::read_to_string(&catalog_path).unwrap();
    let unknown = catalog::execute(
        &CatalogCommands::Model(ModelCommands::Add(ModelAddArgs {
            provider: "absent-provider".to_owned(),
            id: "orphan-model".to_owned(),
            aliases: vec![],
            upstream_model: None,
        })),
        &json_config(),
    )
    .await
    .expect_err("model cannot be added to an absent provider");
    println!("UNKNOWN_PROVIDER_ERROR={unknown:#}");
    assert_eq!(
        std::fs::read_to_string(&catalog_path).unwrap(),
        before_unknown
    );

    let missing_model = catalog::execute(
        &CatalogCommands::Model(ModelCommands::Remove {
            provider: "fixture-openai".to_owned(),
            id: "absent-model".to_owned(),
        }),
        &json_config(),
    )
    .await
    .expect_err("removing an absent model is visible");
    println!("MISSING_MODEL_ERROR={missing_model:#}");
    assert_eq!(
        std::fs::read_to_string(&catalog_path).unwrap(),
        before_unknown
    );

    println!("BEGIN_MODEL_REMOVE");
    catalog::execute(
        &CatalogCommands::Model(ModelCommands::Remove {
            provider: "fixture-openai".to_owned(),
            id: "fixture-model".to_owned(),
        }),
        &json_config(),
    )
    .await
    .expect("remove model");
    println!("END_MODEL_REMOVE");
    let stored = providers(&catalog_path);
    let models = &stored["providers"][0]["models"];
    assert!(
        models.is_null() || models.as_sequence().is_some_and(Vec::is_empty),
        "removed model must not remain in the serialized catalog: {stored:?}"
    );

    println!("BEGIN_PROVIDER_REMOVE");
    catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::Remove {
            name: "fixture-openai".to_owned(),
        }),
        &json_config(),
    )
    .await
    .expect("remove provider");
    println!("END_PROVIDER_REMOVE");
    assert!(
        providers(&catalog_path)["providers"]
            .as_sequence()
            .expect("providers sequence")
            .is_empty()
    );

    let empty_catalog = std::fs::read_to_string(&catalog_path).unwrap();
    let missing_provider = catalog::execute(
        &CatalogCommands::Provider(ProviderCommands::Remove {
            name: "fixture-openai".to_owned(),
        }),
        &json_config(),
    )
    .await
    .expect_err("removing an absent provider is visible");
    println!("MISSING_PROVIDER_ERROR={missing_provider:#}");
    assert_eq!(
        std::fs::read_to_string(&catalog_path).unwrap(),
        empty_catalog
    );

    println!("BEGIN_DISCOVERY_EMPTY");
    catalog::execute(&CatalogCommands::Discovery, &json_config())
        .await
        .expect("missing discovery report is informative");
    println!("END_DISCOVERY_EMPTY");

    use systemprompt_scheduler::jobs::vertex_discovery::VertexDiscoveryJob;
    use systemprompt_traits::{Job, JobContext};
    let context = JobContext::new(
        systemprompt_test_fixtures::fixture_actor(),
        std::sync::Arc::new(()),
        std::sync::Arc::new(()),
        std::sync::Arc::new(()),
    );
    let result = VertexDiscoveryJob
        .execute(&context)
        .await
        .expect("run owned discovery job");
    assert!(result.success, "discovery report job must complete");
    let report = systemprompt_scheduler::jobs::vertex_discovery::latest_report()
        .expect("scheduler publishes its latest discovery report");
    println!("DISCOVERY_RAN_AT={}", report.ran_at);
    assert_eq!(report.failed_publishers.len(), 1, "{report:?}");
    assert!(
        report.failed_publishers[0].starts_with("vertex: service-account key is malformed:"),
        "{report:?}"
    );
    println!("BEGIN_DISCOVERY_PARTIAL");
    catalog::execute(&CatalogCommands::Discovery, &json_config())
        .await
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
    assert!(stdout.contains("Failed to parse"), "{stdout}");
    assert!(stdout.contains("ai/providers.yaml"), "{stdout}");
    assert!(stdout.contains("UNKNOWN_PROVIDER_ERROR="), "{stdout}");
    assert!(stdout.contains("absent-provider"), "{stdout}");
    assert!(stdout.contains("MISSING_MODEL_ERROR="), "{stdout}");
    assert!(stdout.contains("absent-model"), "{stdout}");
    assert!(stdout.contains("MISSING_PROVIDER_ERROR="), "{stdout}");

    for (marker, expected) in [
        (
            "PROVIDER_ADD",
            "Provider fixture-openai (wire openai-chat, surface openai) added",
        ),
        ("MODEL_ADD", "Model fixture-model added to fixture-openai"),
        (
            "MODEL_REMOVE",
            "Model fixture-model removed from fixture-openai",
        ),
        ("PROVIDER_REMOVE", "Provider fixture-openai removed"),
    ] {
        let value = marked_json(&stdout, marker);
        assert_eq!(value["title"], "Provider Registry Updated", "{value}");
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
        item["title"].as_str().unwrap().contains("0 models"),
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
