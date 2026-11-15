//! The reference composition admits `analytics-http` exactly as its
//! conferrals describe: the event and stream routes at `/api/v1/analytics`
//! behind the admin shape, handed the analytics broadcaster; the engagement
//! routes at `/track/engagement` behind the public shape, handed nothing; the
//! `analytics` group with its two grants and the `core content popular` leaf.
//! A route set at the engagement base cannot resolve the broadcaster, while
//! the analytics set can (T10).

use std::sync::Arc;

use clap::Command;
use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, describe_command_line, kernel_commands};
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::cli::{
    CommandDescriptor, DataImpact, DbUrlRouting, KernelGrant, RoutingClass,
};
use systemprompt_extension::handles::AnalyticsBroadcaster;
use systemprompt_extension::router::{ApiBase, BasePath, RouteSet, WebBase};
use systemprompt_extension::{Admitted, Handed, HandleId, RouteGroup};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_runtime::Kernel;
use systemprompt_traits::DynContentCatalogStats;
use systemprompt_traits::events::{AnalyticsSubscription, SubscribedStream};

use crate::probe::{PLAIN, build, kernel, ok};

const OWNER: &str = "analytics-http";

fn admitted() -> Admitted {
    build(reference()).expect("the reference composition admits")
}

fn analytics_sets(admitted: &Admitted) -> Vec<&AdmittedRouteSet> {
    admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == OWNER)
        .collect()
}

fn set_at<'a>(sets: &[&'a AdmittedRouteSet], base: BasePath) -> &'a AdmittedRouteSet {
    sets.iter()
        .copied()
        .find(|set| set.set.base() == base)
        .unwrap_or_else(|| panic!("analytics-http holds a set at {base:?}"))
}

fn routes(set: &AdmittedRouteSet) -> Vec<(String, &'static str)> {
    set.set
        .defs()
        .iter()
        .map(|def| (def.method.to_string(), def.path.as_str()))
        .collect()
}

#[test]
fn analytics_http_holds_exactly_the_two_conferred_route_sets() {
    let admitted = admitted();
    let sets = analytics_sets(&admitted);
    assert_eq!(sets.len(), 2, "{sets:?}");

    let analytics = set_at(&sets, BasePath::Api(ApiBase::Analytics));
    assert_eq!(analytics.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(
        routes(analytics),
        [
            ("POST".to_owned(), "/events"),
            ("POST".to_owned(), "/events/batch"),
            ("GET".to_owned(), "/stream"),
        ]
    );

    let engagement = set_at(&sets, BasePath::Web(WebBase::TrackEngagement));
    assert_eq!(engagement.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(
        routes(engagement),
        [("POST".to_owned(), "/"), ("POST".to_owned(), "/batch")]
    );
}

#[test]
fn only_the_analytics_set_is_handed_the_broadcaster() {
    let admitted = admitted();
    let sets = analytics_sets(&admitted);
    assert_eq!(
        set_at(&sets, BasePath::Api(ApiBase::Analytics)).handles,
        [HandleId::AnalyticsBroadcaster]
    );
    assert!(
        set_at(&sets, BasePath::Web(WebBase::TrackEngagement))
            .handles
            .is_empty()
    );
}

struct NoSubscriptions;

impl AnalyticsSubscription for NoSubscriptions {
    fn subscribe<'a>(
        &'a self,
        _user_id: &'a UserId,
        _connection_id: &'a ConnectionId,
    ) -> futures::future::BoxFuture<'a, Option<SubscribedStream>> {
        Box::pin(std::future::ready(None))
    }
}

fn handing_kernel() -> Kernel {
    kernel().with_handle::<AnalyticsBroadcaster>(Arc::new(NoSubscriptions))
}

fn broadcaster_set(base: BasePath) -> RouteSet {
    RouteSet::at::<Handed<AnalyticsBroadcaster>>(base, RouteGroup::ContentAndMisc)
        .route(http::Method::GET, "/second", ok, PLAIN)
        .build()
}

#[test]
fn a_second_analytics_http_set_requesting_the_broadcaster_cannot_resolve_it() {
    let admitted = admitted();
    let sets = analytics_sets(&admitted);
    let kernel = handing_kernel();
    let engagement = set_at(&sets, BasePath::Web(WebBase::TrackEngagement));
    assert!(
        broadcaster_set(engagement.set.base())
            .materialize(&kernel.route_scoped(&engagement.handles))
            .is_err(),
        "the engagement base confers no handle"
    );
    let analytics = set_at(&sets, BasePath::Api(ApiBase::Analytics));
    assert!(
        broadcaster_set(analytics.set.base())
            .materialize(&kernel.route_scoped(&analytics.handles))
            .is_ok(),
        "negative control: the analytics base hands the broadcaster"
    );
}

#[test]
fn analytics_http_requires_the_content_catalog() {
    let admitted = admitted();
    let member = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == OWNER)
        .expect("analytics-http is admitted");
    let required: Vec<&str> = member
        .sink
        .requires()
        .iter()
        .map(|requirement| requirement.type_name())
        .collect();
    assert!(
        required.contains(&std::any::type_name::<DynContentCatalogStats>()),
        "{required:?}"
    );
}

fn registry() -> CommandRegistry {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn describe(args: &[&str]) -> CommandDescriptor {
    describe_command_line(&registry(), &argv(args)).unwrap_or_else(|e| panic!("{args:?}: {e}"))
}

#[test]
fn analytics_http_declares_its_group_with_its_grants_and_the_popular_leaf() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let owned: Vec<_> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == OWNER)
        .collect();
    let group = owned
        .iter()
        .find(|command| command.decl.path() == ["analytics"])
        .expect("the analytics group is declared");
    assert_eq!(
        group.decl.grants(),
        [KernelGrant::LocalExport, KernelGrant::SkipValidation]
    );
    let leaf = owned
        .iter()
        .find(|command| command.decl.path() == ["core", "content", "popular"])
        .expect("the popular leaf is declared");
    assert!(leaf.decl.grants().is_empty());
    assert_eq!(owned.len(), 2, "{owned:?}");
}

#[test]
fn every_analytics_command_is_a_read_only_local_export_that_preserves_data() {
    for args in [
        &["analytics", "overview"][..],
        &["analytics", "traffic", "pages"],
        &["analytics", "costs"],
        &["analytics", "sessions", "live", "--no-refresh"],
    ] {
        let descriptor = describe(args);
        assert!(descriptor.local_export(), "{args:?}");
        assert!(descriptor.database(), "{args:?}");
        assert!(descriptor.skip_validation(), "{args:?}");
        assert_eq!(
            descriptor.routing_class(),
            RoutingClass::ReadOnly,
            "{args:?}"
        );
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
        assert!(!descriptor.cloud_operator(), "{args:?}");
    }
}

fn resolved_routing(args: &[&str]) -> DbUrlRouting {
    let registry = registry();
    let matches = registry
        .assemble(Command::new("systemprompt"))
        .expect("assemble")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    registry
        .resolve(&matches)
        .expect("resolve")
        .unwrap_or_else(|| panic!("{args:?} resolves to a contributed command"))
        .db_url_routing()
}

#[test]
fn analytics_and_popular_route_database_urls_directly() {
    assert_eq!(
        resolved_routing(&["analytics", "overview"]),
        DbUrlRouting::Direct
    );
    assert_eq!(
        resolved_routing(&["core", "content", "popular", "blog", "--since", "7d"]),
        DbUrlRouting::Direct
    );
}

#[test]
fn popular_resolves_to_its_own_leaf_descriptor() {
    let descriptor = describe(&["core", "content", "popular", "blog", "--since", "7d"]);
    assert!(descriptor.profile() && descriptor.secrets() && descriptor.database());
    assert_eq!(descriptor.routing_class(), RoutingClass::Mutating);
    assert_eq!(descriptor.data_impact(), DataImpact::Preserving);
    assert!(!descriptor.local_export());
    assert!(!descriptor.skip_validation());
    assert!(!descriptor.cloud_operator());
}

#[test]
fn popular_refuses_an_invalid_source_id() {
    let registry = registry();
    let parsed = registry
        .assemble(Command::new("systemprompt"))
        .expect("assemble")
        .try_get_matches_from(["systemprompt", "core", "content", "popular", ""]);
    assert!(parsed.is_err(), "an empty source id is not a SourceId");
}

#[test]
fn the_analytics_group_without_a_subcommand_is_refused() {
    assert!(describe_command_line(&registry(), &argv(&["analytics"])).is_err());
}

#[test]
fn help_lists_analytics_in_its_slot() {
    let help = registry()
        .assemble(Command::new("systemprompt"))
        .expect("assemble")
        .render_help()
        .to_string();
    let position = |name: &str| {
        help.find(&format!("\n  {name} "))
            .unwrap_or_else(|| panic!("`{name}` is listed in --help:\n{help}"))
    };
    assert!(position("admin") < position("analytics"));
    assert!(position("analytics") < position("web"));
}
