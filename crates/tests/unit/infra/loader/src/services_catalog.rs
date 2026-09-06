//! Behavioural tests for the provider catalog and gateway sections of the
//! services tree: include merge, per-file `!include` resolution, provider
//! duplication, gateway-to-registry validation, and the `ServicesBootstrap`
//! cell that fails boot on a bad catalog.

use systemprompt_loader::{ConfigLoadError, ConfigLoader, ServicesBootstrap};
use tempfile::TempDir;

const ROOT: &str = r#"
includes:
  - ../ai/providers.yaml
  - ../ai/gateway.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

const PROVIDERS: &str = r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: anthropic
    models:
      - id: claude-sonnet-4-5
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
          cache_read_per_million: 0.0
"#;

const GATEWAY: &str = r#"
gateway:
  enabled: true
  default_provider: anthropic
  routes:
    - model_pattern: 'claude-*'
      provider: anthropic
  system_prompt_overrides:
    - action: replace
      prompt: '!include override_prompt.txt'
"#;

const OVERRIDE_PROMPT: &str = "terse gateway prompt";

struct Tree {
    _tmp: TempDir,
    root: std::path::PathBuf,
}

fn write_tree(root: &str, providers: &str, gateway: &str) -> Tree {
    let tmp = TempDir::new().expect("tempdir");
    let services = tmp.path().join("services");
    std::fs::create_dir_all(services.join("config")).unwrap();
    std::fs::create_dir_all(services.join("ai")).unwrap();
    std::fs::write(services.join("config/config.yaml"), root).unwrap();
    std::fs::write(services.join("ai/providers.yaml"), providers).unwrap();
    std::fs::write(services.join("ai/gateway.yaml"), gateway).unwrap();
    std::fs::write(services.join("ai/override_prompt.txt"), OVERRIDE_PROMPT).unwrap();
    Tree {
        _tmp: tmp,
        root: services.join("config/config.yaml"),
    }
}

#[test]
fn catalog_and_gateway_merge_from_includes_and_resolve_relative_to_their_file() {
    let tree = write_tree(ROOT, PROVIDERS, GATEWAY);

    let config = ConfigLoader::load_from_path(&tree.root).expect("services tree loads");

    assert_eq!(config.providers.providers.len(), 1);
    assert!(config.providers.find_provider("anthropic").is_some());

    let gateway = config
        .gateway_config()
        .expect("gateway resolved by the loader");
    assert!(gateway.enabled);
    let route = gateway
        .find_route("claude-sonnet-4-5")
        .expect("route matches the catalog model");
    assert_eq!(route.provider.as_str(), "anthropic");
    assert!(
        !route.id.as_str().trim().is_empty(),
        "loader backfills route ids"
    );
    assert_eq!(
        gateway.system_prompt_overrides[0].prompt.as_deref(),
        Some(OVERRIDE_PROMPT),
        "!include resolves beside gateway.yaml, not beside the root"
    );
}

#[test]
fn duplicate_provider_across_includes_is_rejected() {
    let duplicate_root = r#"
includes:
  - ../ai/providers.yaml
  - ../ai/gateway.yaml
  - ../ai/dup.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;
    let tree = write_tree(duplicate_root, PROVIDERS, GATEWAY);
    let ai_dir = tree.root.parent().unwrap().parent().unwrap().join("ai");
    std::fs::write(ai_dir.join("dup.yaml"), PROVIDERS).unwrap();

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("duplicate must fail");

    assert!(
        matches!(err, ConfigLoadError::DuplicateProvider(ref name) if name == "anthropic"),
        "unexpected error: {err:?}"
    );
    assert!(err.to_string().contains("duplicate provider"), "{err}");
}

#[test]
fn gateway_route_naming_an_undeclared_provider_fails_load() {
    let gateway = GATEWAY.replace("provider: anthropic", "provider: ghost");
    let tree = write_tree(ROOT, PROVIDERS, &gateway);

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("undeclared provider");

    assert!(
        matches!(err, ConfigLoadError::Validation(_)),
        "unexpected error: {err:?}"
    );
    assert!(err.to_string().contains("ghost"), "{err}");
}

#[test]
fn services_bootstrap_fails_boot_on_a_bad_catalog() {
    let before = ServicesBootstrap::get().expect_err("nothing initialised yet");
    assert!(
        matches!(before, ConfigLoadError::NotInitialized),
        "unexpected error: {before:?}"
    );
    assert!(!ServicesBootstrap::is_initialized());

    let bad = PROVIDERS.replace("endpoint: https://api.anthropic.com/v1", "endpoint: \"\"");
    let tree = write_tree(ROOT, &bad, GATEWAY);

    let err = ServicesBootstrap::init_from_path(&tree.root).expect_err("empty endpoint");

    assert!(err.to_string().contains("anthropic"), "{err}");
    assert!(
        !ServicesBootstrap::is_initialized(),
        "a failed load must leave the cell empty"
    );
    assert!(matches!(
        ServicesBootstrap::get().expect_err("still uninitialised"),
        ConfigLoadError::NotInitialized
    ));
}
