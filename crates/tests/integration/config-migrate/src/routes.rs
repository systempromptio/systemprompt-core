//! Gateway routes `config migrate` keeps, pins, scopes or refuses.

use systemprompt_cli::commands::admin::config::migrate::{MigrationPlan, plan, write};
use systemprompt_manifest::services::{GatewayRoute, RouteDeployment, SelectionStrategy};

use crate::support::{Tree, from_files};

const TWO: &str = r"
providers:
  - name: alpha
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: alpha
    governance: { european: true }
    models:
      - id: alpha-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
  - name: beta
    wire: anthropic
    surface: anthropic
    endpoint: https://beta.example.com/v1
    api_key_secret: beta
    governance: { european: true }
    models:
      - id: alpha-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
";

fn gateway(body: &str) -> String {
    format!("gateway:\n  enabled: true\n  default_provider: alpha\n{body}")
}

fn tree(gw: &str) -> Tree {
    from_files(&[("providers.yaml", TWO), ("gateway.yaml", &gateway(gw))])
}

fn migrated(tree: &Tree) -> MigrationPlan {
    let plan = plan(&tree.services).expect("plans").expect("pending");
    write::apply(&tree.services, &plan).expect("the migrated tree loads");
    plan
}

fn route<'p>(plan: &'p MigrationPlan, id: &str) -> &'p GatewayRoute {
    plan.document
        .gateway
        .as_ref()
        .expect("gateway")
        .routes
        .iter()
        .find(|r| r.id.as_str() == id)
        .unwrap_or_else(|| panic!("route {id} missing"))
}

fn providers(route: &GatewayRoute) -> Vec<&str> {
    route
        .deployments
        .iter()
        .map(|d| d.endpoint().provider.as_str())
        .collect()
}

fn refused(tree: &Tree, file: &str, key: &str) {
    let error = plan(&tree.services).expect_err("migrate must abort");
    let message = format!("{error:#}");
    assert!(message.contains(file), "{message}");
    assert!(message.contains(key), "{message}");
}

#[test]
fn a_route_with_requirements_is_kept_pinned_to_its_chain() {
    let plan = migrated(&tree(
        "  routes:\n    - id: eu\n      name: EU only\n      model_pattern: 'alpha-*'\n      provider: alpha\n      fallbacks:\n        - provider: beta\n      requires: { european: true }\n",
    ));
    let eu = route(&plan, "eu");
    assert_eq!(eu.label.as_deref(), Some("EU only"));
    assert!(eu.requires.expect("requires").european);
    assert_eq!(providers(eu), ["alpha", "beta"]);
    assert!(!plan.legacy_map.route_models.contains_key("eu"));
}

#[test]
fn a_weighted_route_keeps_its_weights() {
    let plan = migrated(&tree(
        "  routes:\n    - id: split\n      model_pattern: 'alpha-*'\n      provider: alpha\n      weight: 3\n      strategy: weighted\n      fallbacks:\n        - provider: beta\n          weight: 1\n",
    ));
    let split = route(&plan, "split");
    assert_eq!(split.strategy, SelectionStrategy::Weighted);
    let weights: Vec<Option<u32>> = split
        .deployments
        .iter()
        .map(|d| match d {
            RouteDeployment::Weighted(w) => w.weight,
            RouteDeployment::Endpoint(_) => None,
        })
        .collect();
    assert_eq!(weights, [Some(3), Some(1)]);
}

#[test]
fn a_pure_routing_route_folds_into_the_legacy_map() {
    let plan = migrated(&tree(
        "  routes:\n    - id: plain\n      model_pattern: 'alpha-*'\n      provider: alpha\n",
    ));
    assert_eq!(
        plan.legacy_map
            .route_models
            .get("plain")
            .map(String::as_str),
        Some("alpha-*")
    );
}

#[test]
fn a_route_that_matches_no_model_is_dropped_with_a_note() {
    let plan = migrated(&tree(
        "  routes:\n    - id: ghost\n      model_pattern: 'zeta-*'\n      provider: alpha\n      requires: { no_retain: true }\n",
    ));
    assert!(
        plan.notes
            .iter()
            .any(|n| n.contains("route 'ghost' matches no migrated model")),
        "{:?}",
        plan.notes
    );
    assert_eq!(
        plan.legacy_map
            .route_models
            .get("ghost")
            .map(String::as_str),
        Some("zeta-*")
    );
}

#[test]
fn a_differing_scope_chain_becomes_a_scoped_route_and_a_matching_one_is_dropped() {
    let plan = migrated(&tree(
        "  routes:\n    - id: scoped\n      model_pattern: 'alpha-*'\n      provider: alpha\n      fallbacks:\n        - provider: beta\n      by_scope:\n        dimension: project\n        unmapped: shared\n        chains:\n          shop: { provider: beta }\n          same: { provider: alpha, fallbacks: [{ provider: beta }] }\n",
    ));
    let shop = route(&plan, "scoped-project-shop");
    assert_eq!(providers(shop), ["beta"]);
    let when = shop.when.as_ref().expect("when");
    assert_eq!(
        when.scope
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
        [("project", "shop")]
    );
    let routes = &plan.document.gateway.as_ref().expect("gateway").routes;
    assert!(
        !routes
            .iter()
            .any(|r| r.id.as_str() == "scoped-project-same")
    );
}

const THREE: &str = r"
providers:
  - name: alpha
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: alpha
    models:
      - id: alpha-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
  - name: gamma
    wire: anthropic
    surface: anthropic
    endpoint: https://gamma.example.com/v1
    api_key_secret: gamma
    models:
      - id: gamma-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
";

#[test]
fn a_policy_route_cannot_add_a_deployment_the_model_never_reaches() {
    refused(
        &tree(
            "  routes:\n    - id: first\n      model_pattern: 'alpha-*'\n      provider: alpha\n    - id: widen\n      model_pattern: 'alpha-*'\n      provider: beta\n      requires: { european: true }\n",
        ),
        "ai/gateway.yaml",
        "gateway.routes[widen]",
    );
}

#[test]
fn a_scope_chain_cannot_add_a_deployment_either() {
    let gw = gateway(
        "  routes:\n    - id: scoped\n      model_pattern: 'alpha-*'\n      provider: alpha\n      by_scope:\n        dimension: team\n        unmapped: shared\n        chains:\n          ops: { provider: gamma }\n",
    );
    refused(
        &from_files(&[("providers.yaml", THREE), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.routes[scoped-team-ops]",
    );
}

fn aliased(models: &str) -> Tree {
    let providers = format!(
        "providers:\n  - name: alpha\n    wire: anthropic\n    surface: anthropic\n    endpoint: https://api.anthropic.com/v1\n    api_key_secret: alpha\n    models:\n{models}"
    );
    from_files(&[
        ("providers.yaml", &providers),
        ("gateway.yaml", &gateway("")),
    ])
}

#[test]
fn an_alias_becomes_an_alias_route_to_its_model() {
    let plan = migrated(&aliased(
        "      - id: alpha-1\n        aliases: [alpha-latest]\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n",
    ));
    let alias = route(&plan, "alpha-latest-alias");
    let target = alias.alias.as_ref().expect("alias");
    assert_eq!(target.requested.as_str(), "alpha-latest");
    assert_eq!(target.model.as_str(), "alpha-1");
}

#[test]
fn an_alias_that_is_also_a_model_id_aborts() {
    refused(
        &aliased(
            "      - id: alpha-1\n        aliases: [alpha-2]\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n      - id: alpha-2\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n",
        ),
        "ai/providers.yaml",
        "providers[alpha].models[alpha-1].aliases",
    );
}

#[test]
fn an_alias_naming_two_models_aborts() {
    refused(
        &aliased(
            "      - id: alpha-1\n        aliases: [shared]\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n      - id: alpha-2\n        aliases: [shared]\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n",
        ),
        "ai/providers.yaml",
        "models[alpha-2].aliases",
    );
}

#[test]
fn a_default_model_naming_an_alias_resolves_to_its_model() {
    let providers = "providers:\n  - name: alpha\n    wire: anthropic\n    surface: anthropic\n    endpoint: https://api.anthropic.com/v1\n    api_key_secret: alpha\n    models:\n      - id: alpha-1\n        aliases: [alpha-latest]\n        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }\n";
    let plan = migrated(&from_files(&[
        ("providers.yaml", providers),
        ("gateway.yaml", &gateway("  default_model: alpha-latest\n")),
    ]));
    let default = plan
        .document
        .gateway
        .as_ref()
        .and_then(|g| g.default_model.as_ref())
        .expect("default model");
    assert_eq!(default.as_str(), "alpha-1");
}

#[test]
fn a_default_model_nothing_dispatches_aborts() {
    refused(
        &tree("  default_model: nowhere-9\n"),
        "ai/gateway.yaml",
        "gateway.default_model",
    );
}

#[test]
fn a_system_prompt_override_on_an_undeclared_provider_aborts() {
    refused(
        &tree("  system_prompt_overrides:\n    - provider: ghost\n      action: strip\n"),
        "ai/gateway.yaml",
        "gateway.system_prompt_overrides[0].provider",
    );
}

#[test]
fn a_system_prompt_override_keeps_its_provider() {
    let plan = migrated(&tree(
        "  system_prompt_overrides:\n    - provider: beta\n      action: strip\n",
    ));
    let rules = &plan
        .document
        .gateway
        .as_ref()
        .expect("gateway")
        .system_prompt_overrides;
    assert_eq!(rules[0].provider.as_ref().map(|p| p.as_str()), Some("beta"));
}
