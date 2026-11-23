//! The reference composition admits `mcp-http` exactly as its conferral
//! describes: one public set at `ApiBase::McpRegistry` holding only `GET /`,
//! the `plugins mcp` group holding only the `caller-session` grant, and
//! requirements on the MCP server registry and the read-only
//! `McpServiceObserver` the `mcp` member provides. A profile disabling
//! `mcp-http` refuses `plugins mcp` at dispatch; one disabling `mcp` refuses
//! boot. No install holds the `plugins mcp` path or its grant. No install
//! holds any path under `/api/v1/mcp`, the registry path included: a literal
//! sibling or child of the registry path, a parameter or catch-all reaching
//! the proxy's routes, a service prefix, an extension base and the registry
//! base itself are refused at admission.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use systemprompt::{CompositionError, reference};
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{
    CommandRegistry, ResolvedCommand, describe_command_line, kernel_commands,
};
use systemprompt_cli::runner::refuse_disabled;
use systemprompt_extension::cli::{
    CommandDecl, CommandDescriptor, DataImpact, KernelGrant, parents,
};
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, RouteSet, RouteSpec, WebBase,
};
use systemprompt_extension::{Admitted, AuthzPolicy, ContextKind, ContributionError, RouteGroup};
use systemprompt_mcp::services::McpServiceObserver;
use systemprompt_traits::McpRegistryProvider;

use crate::cli_probe::McpGroup;
use crate::probe::{PLAIN, Probe, ProbeState, build, build_with, ok, plain_set, profile, shape};

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
        [
            std::any::type_name::<Arc<dyn McpRegistryProvider>>(),
            std::any::type_name::<Arc<McpServiceObserver>>(),
        ]
    );
}

fn registry() -> CommandRegistry {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn describe(args: &[&str]) -> CommandDescriptor {
    let argv: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    describe_command_line(&registry(), &argv).unwrap_or_else(|e| panic!("{args:?}: {e}"))
}

fn resolved(args: &[&str]) -> ResolvedCommand {
    let registry = registry();
    let matches = systemprompt_cli::args::command(&registry)
        .expect("the reference commands attach")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .expect("the command parses");
    registry
        .resolve(&matches)
        .expect("the command resolves")
        .expect("the command is a contributed command")
}

#[test]
fn mcp_http_declares_plugins_mcp_with_only_the_services_tree_and_caller_session_grants() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let owned: Vec<(Vec<&str>, &[KernelGrant])> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == OWNER)
        .map(|command| (command.decl.path(), command.decl.grants()))
        .collect();
    assert_eq!(
        owned,
        [(
            vec!["plugins", "mcp"],
            [KernelGrant::ServicesTree, KernelGrant::CallerSession].as_slice()
        )]
    );
}

#[test]
fn every_plugins_mcp_command_preserves_data() {
    for args in [
        ["plugins", "mcp", "list"].as_slice(),
        &["plugins", "mcp", "list-packages"],
        &["plugins", "mcp", "status"],
        &["plugins", "mcp", "validate", "--all"],
        &["plugins", "mcp", "logs", "--disk"],
        &["plugins", "mcp", "tools"],
        &["plugins", "mcp", "call", "server", "tool"],
    ] {
        let descriptor = describe(args);
        assert!(!descriptor.is_destructive(), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
        assert!(descriptor.database(), "{args:?}");
    }
}

#[test]
fn a_profile_disabling_mcp_http_refuses_plugins_mcp_at_dispatch() {
    let command = resolved(&["plugins", "mcp", "status"]);
    let error = refuse_disabled(&command, &profile(&[OWNER]))
        .expect_err("the profile disables the command's owner");
    assert!(matches!(
        error,
        CliRunError::CommandDisabled { path, owner }
            if path == ["plugins", "mcp"] && owner.as_str() == OWNER
    ));
    assert!(
        refuse_disabled(&command, &profile(&[])).is_ok(),
        "negative control"
    );
}

#[test]
fn a_profile_disabling_mcp_while_mcp_http_stays_enabled_refuses_boot() {
    let error = reference()
        .build(&profile(&["mcp"]))
        .expect_err("mcp-http requires values only mcp provides");
    assert!(
        error.to_string().contains("mcp"),
        "the refusal names the disabled provider: {error}"
    );
    assert!(
        reference().build(&profile(&[OWNER])).is_ok(),
        "negative control: disabling mcp-http alone boots"
    );
}

#[test]
fn an_install_claiming_plugins_mcp_is_refused() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.command(CommandDecl::group_under::<parents::Plugins, McpGroup>());
    }))
    .expect_err("plugins mcp is reserved to mcp-http");
    assert!(
        matches!(
            error,
            CompositionError::ReservedCliPath {
                path: ["plugins", "mcp"],
                ..
            }
        ),
        "{error:?}"
    );

    let error = build_with(Probe::new("probe-install", |c| {
        c.command(
            CommandDecl::group_under::<parents::Plugins, McpGroup>()
                .requesting(&[KernelGrant::CallerSession]),
        );
    }))
    .expect_err("an install holds neither the path nor its grant");
    assert!(
        matches!(
            error,
            CompositionError::ReservedCliPath { .. } | CompositionError::GrantNotConferred { .. }
        ),
        "{error:?}"
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
