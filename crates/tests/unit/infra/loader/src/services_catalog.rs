//! Behavioural tests for the model catalog the services loader reads by
//! convention from `services/ai/`: the five files merge, a gateway prompt
//! `!include` resolves beside `gateway.yaml`, a duplicate id is refused, a
//! catalog section outside the convention is refused, a legacy tree is refused
//! with the migrate hint, and `ServicesState` fails boot on a bad catalog.

use systemprompt_loader::{ConfigLoadError, ConfigLoader, ServicesState};
use tempfile::TempDir;

const ROOT: &str = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

const CREDENTIALS: &str = r#"
credentials:
  anthropic: { kind: platform, secret: anthropic }
"#;

const PROVIDERS: &str = r#"
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api: { wire: anthropic, url: "https://api.anthropic.com/v1" }
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

const DEPLOYMENTS: &str = r#"
deployments:
  claude-sonnet-4-5: [anthropic/api]
"#;

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
      prompt: '!include override_prompt.txt'
"#;

const OVERRIDE_PROMPT: &str = "terse gateway prompt";

struct Tree {
    _tmp: TempDir,
    root: std::path::PathBuf,
    ai: std::path::PathBuf,
}

fn write_tree(root: &str, files: &[(&str, &str)]) -> Tree {
    let tmp = TempDir::new().expect("tempdir");
    let services = tmp.path().join("services");
    std::fs::create_dir_all(services.join("config")).unwrap();
    std::fs::create_dir_all(services.join("ai")).unwrap();
    std::fs::write(services.join("config/config.yaml"), root).unwrap();
    for (name, body) in files {
        std::fs::write(services.join("ai").join(name), body).unwrap();
    }
    std::fs::write(services.join("ai/override_prompt.txt"), OVERRIDE_PROMPT).unwrap();
    Tree {
        _tmp: tmp,
        root: services.join("config/config.yaml"),
        ai: services.join("ai"),
    }
}

fn catalog_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("credentials.yaml", CREDENTIALS),
        ("providers.yaml", PROVIDERS),
        ("models.yaml", MODELS),
        ("deployments.yaml", DEPLOYMENTS),
        ("gateway.yaml", GATEWAY),
    ]
}

#[test]
fn the_catalog_is_read_by_convention_and_includes_resolve_beside_gateway_yaml() {
    let tree = write_tree(ROOT, &catalog_files());

    let config = ConfigLoader::load_from_path(&tree.root).expect("services tree loads");

    assert_eq!(config.providers.providers.len(), 1);
    assert!(config.models.get("claude-sonnet-4-5").is_some());
    let gateway = config.gateway_config().expect("gateway resolved");
    assert!(gateway.enabled);
    assert!(gateway.find_route("sonnet-pin").is_some());
    assert_eq!(
        gateway.system_prompt_overrides[0].prompt.as_deref(),
        Some(OVERRIDE_PROMPT),
        "!include resolves beside gateway.yaml"
    );
}

#[test]
fn an_id_declared_in_two_catalog_files_is_refused() {
    let mut files = catalog_files();
    let doubled = format!("{CREDENTIALS}{}", PROVIDERS.trim_start());
    files[0] = ("credentials.yaml", Box::leak(doubled.into_boxed_str()));
    let tree = write_tree(ROOT, &files);

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("duplicate must fail");
    assert!(err.to_string().contains("anthropic"), "{err}");
}

#[test]
fn an_include_naming_a_catalog_file_is_refused() {
    let root = format!("includes:\n  - ../ai/providers.yaml\n{ROOT}");
    let tree = write_tree(&root, &catalog_files());

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("outside convention");
    assert!(
        matches!(err, ConfigLoadError::CatalogOutsideConvention { .. }),
        "unexpected error: {err:?}"
    );
    assert!(err.to_string().contains("config migrate"), "{err}");
}

#[test]
fn a_legacy_provider_list_is_refused_with_the_migrate_hint() {
    let legacy = "providers:\n  - name: anthropic\n    wire: anthropic\n";
    let tree = write_tree(ROOT, &[("providers.yaml", legacy)]);

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("legacy refused");
    assert!(
        matches!(err, ConfigLoadError::LegacyCatalog { .. }),
        "unexpected error: {err:?}"
    );
    assert!(err.to_string().contains("config migrate"), "{err}");
    assert!(tree.ai.join("providers.yaml").exists());
}

#[test]
fn a_route_pinning_an_undeployed_endpoint_fails_load() {
    let mut files = catalog_files();
    files[4] = (
        "gateway.yaml",
        "gateway:\n  enabled: true\n  routes:\n    - id: ghost\n      models: [\"claude-*\"]\n      deployments: [ghost/api]\n",
    );
    let tree = write_tree(ROOT, &files);

    let err = ConfigLoader::load_from_path(&tree.root).expect_err("undeployed endpoint");
    assert!(err.to_string().contains("ghost"), "{err}");
}

#[test]
fn services_state_fails_boot_on_a_bad_catalog() {
    let mut files = catalog_files();
    files[1] = (
        "providers.yaml",
        "providers:\n  anthropic:\n    credential: anthropic\n    endpoints:\n      api: { wire: anthropic, url: \"\" }\n",
    );
    let tree = write_tree(ROOT, &files);

    let err = ServicesState::load_from_path(&tree.root).expect_err("empty endpoint url");

    assert!(err.to_string().contains("anthropic"), "{err}");
}
