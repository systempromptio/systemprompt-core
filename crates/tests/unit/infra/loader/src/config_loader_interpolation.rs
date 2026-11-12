use std::path::PathBuf;

use systemprompt_loader::{ConfigLoadError, ConfigLoader};
use tempfile::TempDir;

const ROOT: &str = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

const CREDENTIALS: &str = "credentials:\n  anthropic: { kind: platform, secret: anthropic }\n";

const PROVIDERS: &str = r#"
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api: { wire: anthropic, url: "https://api.anthropic.com/${WPF_API_PATH}" }
"#;

const MODELS: &str = r#"
models:
  claude-sonnet-4-5:
    clients: [claude]
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
"#;

const DEPLOYMENTS: &str = "deployments:\n  claude-sonnet-4-5: [anthropic/api]\n";

const GATEWAY: &str = r#"
gateway:
  enabled: true
  default_model: claude-sonnet-4-5
  routes:
    - id: sonnet-pin
      models: ["claude-*"]
      deployments: [anthropic/api]
  system_prompt_overrides:
    - action: replace
      prompt: "Repeat ${WPF_UNSET} verbatim"
"#;

const PROXY_URL: &str = "https://api.anthropic.com/proxy/v1";

fn fixture_env(name: &str) -> Option<String> {
    (name == "WPF_API_PATH").then(|| "proxy/v1".to_owned())
}

fn empty_env(_name: &str) -> Option<String> {
    None
}

struct Tree {
    _tmp: TempDir,
    root: PathBuf,
}

fn write_tree(root: &str, extra: &[(&str, &str)]) -> Tree {
    let tmp = TempDir::new().expect("tempdir");
    let services = tmp.path().join("services");
    std::fs::create_dir_all(services.join("config")).expect("config dir");
    std::fs::create_dir_all(services.join("ai")).expect("ai dir");
    std::fs::write(services.join("config/config.yaml"), root).expect("root");
    for (name, body) in [
        ("credentials.yaml", CREDENTIALS),
        ("providers.yaml", PROVIDERS),
        ("models.yaml", MODELS),
        ("deployments.yaml", DEPLOYMENTS),
        ("gateway.yaml", GATEWAY),
    ] {
        std::fs::write(services.join("ai").join(name), body).expect("catalog file");
    }
    for (name, body) in extra {
        std::fs::write(services.join("config").join(name), body).expect("extra file");
    }
    Tree {
        _tmp: tmp,
        root: services.join("config/config.yaml"),
    }
}

#[test]
fn a_provider_endpoint_placeholder_resolves_and_prompt_text_stays_literal() {
    let tree = write_tree(ROOT, &[]);

    let config = ConfigLoader::new(tree.root.clone())
        .with_env_lookup(fixture_env)
        .load()
        .expect("services tree loads");

    let providers = format!("{:?}", config.providers);
    assert!(providers.contains(PROXY_URL), "{providers}");
    assert!(!providers.contains("WPF_API_PATH"), "{providers}");
    let gateway = config.gateway_config().expect("gateway resolved");
    assert_eq!(
        gateway.system_prompt_overrides[0].prompt.as_deref(),
        Some("Repeat ${WPF_UNSET} verbatim")
    );
}

#[test]
fn a_missing_variable_fails_the_load_naming_the_variable_and_file() {
    let tree = write_tree(ROOT, &[]);

    let err = ConfigLoader::new(tree.root.clone())
        .with_env_lookup(empty_env)
        .load()
        .expect_err("unset variable must fail");

    let ConfigLoadError::Interpolation { path, source } = &err else {
        panic!("unexpected error: {err:?}");
    };
    assert!(path.ends_with("ai/providers.yaml"), "{}", path.display());
    assert_eq!(source.variable, "WPF_API_PATH");
    assert!(err.to_string().contains("WPF_API_PATH"), "{err}");
}

#[test]
fn inspection_loads_with_the_placeholder_left_as_text() {
    let tree = write_tree(ROOT, &[]);

    let config = ConfigLoader::new(tree.root.clone())
        .with_env_lookup(empty_env)
        .inspecting()
        .load()
        .expect("inspection tolerates an unset variable");

    let providers = format!("{:?}", config.providers);
    assert!(providers.contains("${WPF_API_PATH}"), "{providers}");
}

#[test]
fn verbatim_load_keeps_the_placeholder_even_when_the_variable_is_set() {
    let tree = write_tree(ROOT, &[]);

    let config = ConfigLoader::new(tree.root.clone())
        .with_env_lookup(fixture_env)
        .verbatim()
        .load()
        .expect("verbatim load");

    let providers = format!("{:?}", config.providers);
    assert!(providers.contains("${WPF_API_PATH}"), "{providers}");
    assert!(!providers.contains(PROXY_URL), "{providers}");
}

#[test]
fn an_included_fragment_is_interpolated() {
    let root = format!("includes:\n  - fragment.yaml\n{ROOT}");
    let fragment = "agents: {}\nmcp_servers: {}\nunused: \"${WPF_FRAGMENT_VAR}\"\n";
    let tree = write_tree(&root, &[("fragment.yaml", fragment)]);

    let err = ConfigLoader::new(tree.root.clone())
        .with_env_lookup(fixture_env)
        .load()
        .expect_err("the fragment's unset variable must fail");

    let ConfigLoadError::Interpolation { path, source } = &err else {
        panic!("unexpected error: {err:?}");
    };
    assert!(path.ends_with("fragment.yaml"), "{}", path.display());
    assert_eq!(source.variable, "WPF_FRAGMENT_VAR");
}
