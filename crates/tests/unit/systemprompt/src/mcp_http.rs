//! The reference composition admits `mcp-http` exactly as its conferral
//! describes: one public set at `ApiBase::McpRegistry` holding only `GET /`,
//! requiring the MCP server registry the `mcp` member provides. No install
//! holds any path under `/api/v1/mcp`, the registry path included: a literal
//! sibling or child of the registry path, a parameter or catch-all reaching
//! the proxy's routes, a service prefix, an extension base and the registry
//! base itself are refused at admission.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, RouteSet, RouteSpec, WebBase,
};
use systemprompt_extension::{Admitted, AuthzPolicy, ContextKind, ContributionError, RouteGroup};
use systemprompt_traits::McpRegistryProvider;

use crate::probe::{PLAIN, Probe, ProbeState, build, build_with, ok, plain_set, shape};

const OWNER: &str = CoreId::McpHttp.id();
const REGISTRY: BasePath = BasePath::Api(ApiBase::McpRegistry);
const ROOT: BasePath = BasePath::Web(WebBase::Root);

fn admitted() -> Admitted {
    build(reference()).expect("the reference composition admits")
}

#[test]
fn mcp_http_holds_the_one_pinned_registry_route() {
    let admitted = admitted();
    let sets: Vec<_> = admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == OWNER)
        .collect();
    assert_eq!(sets.len(), 1, "{sets:?}");
    let set = sets[0];
    assert_eq!(set.set.base(), BaseRef::Path(REGISTRY));
    assert_eq!(set.set.group(), RouteGroup::McpAndStream);
    assert!(set.handles.is_empty(), "mcp-http is handed nothing");
    let defs: Vec<(String, &str, AuthzPolicy, ContextKind)> = set
        .set
        .defs()
        .iter()
        .map(|def| {
            (
                def.method.to_string(),
                def.path.as_str(),
                def.spec.policy,
                def.spec.context,
            )
        })
        .collect();
    assert_eq!(
        defs,
        [(
            "GET".to_owned(),
            "/",
            AuthzPolicy::public(),
            ContextKind::Public
        )]
    );
}

#[test]
fn mcp_http_requires_the_mcp_server_registry() {
    let admitted = admitted();
    let member = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == OWNER)
        .expect("mcp-http is admitted");
    let required: Vec<&str> = member
        .sink
        .requires()
        .iter()
        .map(|requirement| requirement.type_name())
        .collect();
    assert_eq!(
        required,
        [std::any::type_name::<Arc<dyn McpRegistryProvider>>()]
    );
}

fn refused_at(result: Result<Admitted, CompositionError>) -> &'static str {
    match result {
        Err(CompositionError::ReservedPrefix { prefix, .. }) => prefix,
        Err(error) => panic!("expected a reserved-path refusal, got {error:?}"),
        Ok(_) => panic!("expected a reserved-path refusal, the def was admitted"),
    }
}

#[test]
fn an_install_path_under_the_mcp_base_is_refused() {
    for path in [
        "/api/v1/mcp/registry",
        "/api/v1/mcp/registryx",
        "/api/v1/mcp/registry/x",
        "/api/v1/mcp/{service_name}/mcp",
        "/api/v1/mcp/{*rest}",
    ] {
        let result = build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(ROOT, path));
        }));
        assert_eq!(refused_at(result), "/api/v1/mcp", "{path}");
    }
}

#[test]
fn an_install_service_at_the_registry_path_is_refused() {
    let result = build_with(Probe::new("probe-install", |c| {
        c.routes(
            RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                .service(
                    "/api/v1/mcp/registry",
                    Router::new().route("/", get(ok)),
                    PLAIN,
                )
                .build(),
        );
    }));
    assert_eq!(refused_at(result), "/api/v1/mcp");
}

#[test]
fn an_install_extension_base_at_the_registry_path_is_refused() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.routes(plain_set(
            BasePath::Api(ApiBase::Extension("/api/v1/mcp/registry")),
            "/x",
        ));
    }))
    .expect_err("an extension base inside the kernel's MCP namespace is refused");
    assert!(
        matches!(
            error,
            CompositionError::Contribution {
                source: ContributionError::InvalidBase(_),
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn an_install_at_the_registry_base_with_its_shape_is_refused() {
    let conferred = shape(
        CoreId::McpHttp,
        BaseRef::Path(REGISTRY),
        Allowance::ReservedPrefix,
    );
    let spec = RouteSpec::new(
        conferred.shape.policy,
        conferred.shape.context,
        conferred.shape.limits,
    );
    let result = build_with(Probe::new("registry-impostor", move |c| {
        c.routes(
            RouteSet::at::<ProbeState>(REGISTRY, conferred.group)
                .route(http::Method::GET, "/", ok, spec)
                .build(),
        );
    }));
    assert_eq!(refused_at(result), "/api/v1/mcp");
}
