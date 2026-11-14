//! The built-in gateway prefix list is complete by construction: every route
//! the built-in gateway surface declares, read from the router source by
//! [`router_inventory`], lies under a prefix of `builtin_gateway_prefixes()`,
//! and every prefix covers at least one such route.
//!
//! The inventory gives each `.route(` call's site (`file::fn`) and its path
//! argument as source text. The built-in surface is every site under
//! `routes/gateway/` except the public session router (`sessions.rs`, mounted
//! at the public gateway base), plus the bridge drive router
//! (`routes/documents/drive.rs::router`) the gateway merges. A path argument
//! is a string literal, an `ApiPaths` constant, or
//! `ApiPaths::gateway_relative(ApiPaths::…)`; constants are resolved from the
//! `ApiPaths` source itself, so no route or path is listed a second time.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use systemprompt_api::routes::gateway::builtin::builtin_gateway_prefixes;
use systemprompt_api::services::middleware::path_under;
use systemprompt_models::modules::ApiPaths;

use super::inventory::{RouteKey, router_inventory};

fn api_path_constants() -> BTreeMap<String, String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../shared/models/src/modules/api_paths.rs");
    let text = fs::read_to_string(&source).expect("read ApiPaths source");
    let mut constants = BTreeMap::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("pub const ") {
        rest = &rest[at + "pub const ".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let end = rest.find(';').expect("const ends");
        let declaration = &rest[..end];
        if declaration.contains("&'static str") {
            let value = declaration
                .split('"')
                .nth(1)
                .unwrap_or_else(|| panic!("{name} has a string value"));
            constants.insert(name, value.to_owned());
        }
        rest = &rest[end..];
    }
    constants
}

fn resolve(path_arg: &str, constants: &BTreeMap<String, String>) -> String {
    let constant = |expr: &str| -> String {
        let name = expr
            .strip_prefix("ApiPaths::")
            .unwrap_or_else(|| panic!("unresolvable gateway route path `{path_arg}`"));
        constants
            .get(name)
            .unwrap_or_else(|| panic!("ApiPaths::{name} not found"))
            .clone()
    };
    if let Some(literal) = path_arg.strip_prefix('"').and_then(|p| p.strip_suffix('"')) {
        literal.to_owned()
    } else if let Some(inner) = path_arg
        .strip_prefix("ApiPaths::gateway_relative(")
        .and_then(|p| p.strip_suffix(')'))
    {
        let absolute = constant(inner);
        absolute
            .strip_prefix(ApiPaths::GATEWAY_BASE)
            .unwrap_or(&absolute)
            .to_owned()
    } else {
        constant(path_arg)
    }
}

fn on_builtin_surface(key: &RouteKey) -> bool {
    (key.site.starts_with("routes/gateway/") && !key.site.starts_with("routes/gateway/sessions.rs"))
        || key.site == "routes/documents/drive.rs::router"
}

fn builtin_routes() -> Vec<(RouteKey, String)> {
    let constants = api_path_constants();
    let routes: Vec<(RouteKey, String)> = router_inventory()
        .into_iter()
        .filter(on_builtin_surface)
        .map(|key| {
            let path = resolve(&key.path, &constants);
            (key, path)
        })
        .collect();
    assert!(
        routes.len() > 20,
        "the built-in gateway surface was found: {} routes",
        routes.len()
    );
    routes
}

#[test]
fn every_builtin_gateway_route_lies_under_a_listed_prefix() {
    let prefixes = builtin_gateway_prefixes();
    let uncovered: Vec<String> = builtin_routes()
        .into_iter()
        .filter(|(_, path)| !prefixes.iter().any(|prefix| path_under(path, prefix)))
        .map(|(key, path)| format!("{key} -> {path}"))
        .collect();
    assert!(
        uncovered.is_empty(),
        "built-in gateway routes outside every builtin_gateway_prefixes() entry would answer \
         404: {uncovered:#?}"
    );
}

#[test]
fn every_listed_prefix_covers_a_builtin_gateway_route() {
    let routes = builtin_routes();
    let dead: Vec<&str> = builtin_gateway_prefixes()
        .into_iter()
        .filter(|prefix| !routes.iter().any(|(_, path)| path_under(path, prefix)))
        .collect();
    assert!(dead.is_empty(), "prefixes that cover no built-in route: {dead:?}");
}
