//! The built-in gateway prefix list is complete by construction: every route
//! the built-in gateway surface declares, read from the router source, lies
//! under a prefix of `builtin_gateway_prefixes()`, and every prefix covers at
//! least one such route.
//!
//! The surface is every site under `routes/gateway/` except the public
//! session router (`sessions.rs`, mounted at the public gateway base), plus
//! the bridge drive router (`routes/documents/drive.rs::router`) the gateway
//! merges. Its routes are every `.route(` call there, read for the path
//! argument alone, so a route whose method router is built by a function call
//! is still seen. A path argument is a string literal, an `ApiPaths`
//! constant, or `ApiPaths::gateway_relative(ApiPaths::…)`; constants are
//! resolved from the `ApiPaths` source itself, so no route or path is listed
//! a second time, and any other path expression fails the test naming its
//! site.
//!
//! Any other way a surface router gains routes fails the test unless the scan
//! sees it: a `.nest(`, `.nest_service(`, `.route_service(`, `.fallback(` or
//! `.fallback_service(` on the surface fails outright, and a `.merge(` of a
//! call must call a surface router function or a function whose body calls
//! one. A `.merge(` of a binding or field merges a router already built from
//! such calls, or a registered group confined to its own prefix separately.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use systemprompt_api::routes::gateway::builtin::builtin_gateway_prefixes;
use systemprompt_api::services::middleware::path_under;
use systemprompt_models::modules::ApiPaths;

use super::inventory::{Source, each_call, enclosing_fn, fn_body, sources, split_first_arg};

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

fn resolve(site: &str, path_arg: &str, constants: &BTreeMap<String, String>) -> String {
    let constant = |expr: &str| -> String {
        let name = expr
            .strip_prefix("ApiPaths::")
            .unwrap_or_else(|| panic!("{site}: unresolvable gateway route path `{path_arg}`"));
        constants
            .get(name)
            .unwrap_or_else(|| panic!("{site}: ApiPaths::{name} not found"))
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

#[derive(Debug, Clone)]
struct MountCall {
    site: String,
    kind: &'static str,
    arg: String,
}

const MOUNT_CALLS: [&str; 6] = [
    ".merge(",
    ".nest(",
    ".nest_service(",
    ".route_service(",
    ".fallback(",
    ".fallback_service(",
];

fn route_paths(sources: &[Source]) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    for (rel, code) in sources {
        each_call(code, ".route(", |open, inner| {
            let (path_arg, _) = split_first_arg(inner);
            let site = format!("{rel}::{}", enclosing_fn(&code[..open]));
            paths.push((site, path_arg.split_whitespace().collect()));
        });
    }
    paths
}

fn mount_calls(sources: &[Source]) -> Vec<MountCall> {
    let mut calls = Vec::new();
    for (rel, code) in sources {
        for kind in MOUNT_CALLS {
            each_call(code, kind, |open, inner| {
                calls.push(MountCall {
                    site: format!("{rel}::{}", enclosing_fn(&code[..open])),
                    kind,
                    arg: inner.split_whitespace().collect(),
                });
            });
        }
    }
    calls
}

fn on_builtin_surface(site: &str) -> bool {
    (site.starts_with("routes/gateway/") && !site.starts_with("routes/gateway/sessions.rs"))
        || site == "routes/documents/drive.rs::router"
}

fn builtin_routes(sources: &[Source]) -> Vec<(String, String)> {
    let constants = api_path_constants();
    let routes: Vec<(String, String)> = route_paths(sources)
        .into_iter()
        .filter(|(site, _)| on_builtin_surface(site))
        .map(|(site, path_arg)| {
            let path = resolve(&site, &path_arg, &constants);
            (format!("{site} {path_arg}"), path)
        })
        .collect();
    assert!(
        routes.len() > 20,
        "the built-in gateway surface was found: {} routes",
        routes.len()
    );
    routes
}

fn surface_router_fns(sources: &[Source]) -> Vec<String> {
    route_paths(sources)
        .into_iter()
        .filter(|(site, _)| on_builtin_surface(site))
        .filter_map(|(site, _)| site.rsplit("::").next().map(str::to_owned))
        .collect()
}

fn merged_call_is_scanned(sources: &[Source], router_fns: &[String], arg: &str) -> bool {
    let callee_of = |call: &str| -> Option<String> {
        let head = call.split('(').next()?;
        Some(head.rsplit("::").next().unwrap_or(head).to_owned())
    };
    let Some(callee) = callee_of(arg) else {
        return false;
    };
    if router_fns.contains(&callee) {
        return true;
    }
    fn_body(sources, &callee).is_some_and(|(_, body)| {
        router_fns
            .iter()
            .any(|router_fn| body.contains(&format!("{router_fn}(")))
    })
}

#[test]
fn every_way_the_builtin_surface_gains_routes_is_scanned() {
    let sources = sources();
    let router_fns = surface_router_fns(&sources);
    let unseen: Vec<String> = mount_calls(&sources)
        .into_iter()
        .filter(|call| on_builtin_surface(&call.site))
        .filter(|call| {
            let is_call = call.arg.contains('(');
            call.kind != ".merge("
                || (is_call && !merged_call_is_scanned(&sources, &router_fns, &call.arg))
        })
        .map(|call| format!("{} {}{})", call.site, call.kind, call.arg))
        .collect();
    assert!(
        unseen.is_empty(),
        "router calls on the built-in gateway surface the prefix scan cannot see: {unseen:#?}"
    );
}

#[test]
fn every_builtin_gateway_route_lies_under_a_listed_prefix() {
    let prefixes = builtin_gateway_prefixes();
    let uncovered: Vec<String> = builtin_routes(&sources())
        .into_iter()
        .filter(|(_, path)| !prefixes.iter().any(|prefix| path_under(path, prefix)))
        .map(|(site, path)| format!("{site} -> {path}"))
        .collect();
    assert!(
        uncovered.is_empty(),
        "built-in gateway routes outside every builtin_gateway_prefixes() entry would answer \
         404: {uncovered:#?}"
    );
}

#[test]
fn every_listed_prefix_covers_a_builtin_gateway_route() {
    let routes = builtin_routes(&sources());
    let dead: Vec<&str> = builtin_gateway_prefixes()
        .into_iter()
        .filter(|prefix| !routes.iter().any(|(_, path)| path_under(path, prefix)))
        .collect();
    assert!(
        dead.is_empty(),
        "prefixes that cover no built-in route: {dead:?}"
    );
}
