//! The reference composition admits `content-http` exactly as its conferrals
//! describe: the public reads at `/api/v1/content` behind the public shape,
//! the signed-in set holding only `POST /links/generate` behind the user
//! shape, and the short-link redirect `GET /r/{short_code}` at the web root
//! behind the public shape (T16, facade side: each real set is admitted under
//! its own conferral; drift refusal is the kernel re-check's, since a probe
//! cannot hold core standing). It declares the `core content` and
//! `web sitemap` groups with no grants, classified as the CLI router reads
//! them: deleting content, a source or a link is destructive, the read
//! commands preserve data, `core content` reads the database and
//! `web sitemap` the profile paths.

use std::sync::Arc;

use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, describe_command_line, kernel_commands};
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::cli::{CommandDescriptor, DataImpact};
use systemprompt_extension::router::{ApiBase, BasePath, WebBase};
use systemprompt_extension::{Admitted, AuthzPolicy, ContextKind, RouteGroup};

use crate::probe::build;

const OWNER: &str = "content-http";

fn admitted() -> Admitted {
    build(reference()).expect("the reference composition admits")
}

fn content_sets(admitted: &Admitted) -> Vec<&AdmittedRouteSet> {
    admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == OWNER)
        .collect()
}

fn routes(set: &AdmittedRouteSet) -> Vec<(String, &'static str)> {
    set.set
        .defs()
        .iter()
        .map(|def| (def.method.to_string(), def.path.as_str()))
        .collect()
}

fn policies(set: &AdmittedRouteSet) -> Vec<(AuthzPolicy, ContextKind)> {
    set.set
        .defs()
        .iter()
        .map(|def| (def.spec.policy, def.spec.context))
        .collect()
}

#[test]
fn content_http_holds_the_three_conferred_route_sets() {
    let admitted = admitted();
    let sets = content_sets(&admitted);
    assert_eq!(sets.len(), 3, "{sets:?}");
    for set in &sets {
        assert_eq!(set.set.group(), RouteGroup::ContentAndMisc);
        assert!(set.handles.is_empty(), "content-http is handed nothing");
    }

    let at_content: Vec<&&AdmittedRouteSet> = sets
        .iter()
        .filter(|set| set.set.base() == BasePath::Api(ApiBase::Content))
        .collect();
    assert_eq!(at_content.len(), 2, "{at_content:?}");

    let signed_in = at_content
        .iter()
        .find(|set| {
            policies(set)
                .iter()
                .all(|shape| *shape == (AuthzPolicy::user(), ContextKind::User))
        })
        .expect("a signed-in set at /api/v1/content");
    assert_eq!(routes(signed_in), [("POST".to_owned(), "/links/generate")]);

    let public = at_content
        .iter()
        .find(|set| {
            policies(set)
                .iter()
                .all(|shape| *shape == (AuthzPolicy::public(), ContextKind::Public))
        })
        .expect("a public set at /api/v1/content");
    let public_routes = routes(public);
    assert!(
        !public_routes
            .iter()
            .any(|(_, path)| *path == "/links/generate"),
        "{public_routes:?}"
    );
    for expected in [
        ("POST", "/query"),
        ("GET", "/{source_id}"),
        ("GET", "/{source_id}/{slug}"),
        ("GET", "/{source_id}/{slug}/{suffix}"),
        ("GET", "/links"),
        ("GET", "/links/journey"),
    ] {
        assert!(
            public_routes
                .iter()
                .any(|(method, path)| method == expected.0 && *path == expected.1),
            "{expected:?} in {public_routes:?}"
        );
    }

    let redirect = sets
        .iter()
        .find(|set| set.set.base() == BasePath::Web(WebBase::Root))
        .expect("the redirect set at the web root");
    assert_eq!(routes(redirect), [("GET".to_owned(), "/r/{short_code}")]);
    assert_eq!(
        policies(redirect),
        [(AuthzPolicy::public(), ContextKind::Public)]
    );
}

#[test]
fn content_http_requires_the_content_repositories() {
    let admitted = admitted();
    let member = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == OWNER)
        .expect("content-http is admitted");
    let required: Vec<&str> = member
        .sink
        .requires()
        .iter()
        .map(|requirement| requirement.type_name())
        .collect();
    assert_eq!(
        required,
        [std::any::type_name::<
            Arc<systemprompt::content::ContentRepositories>,
        >()]
    );
}

#[test]
fn content_http_declares_core_content_and_web_sitemap_without_grants() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let mut owned: Vec<(Vec<&str>, usize)> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == OWNER)
        .map(|command| (command.decl.path(), command.decl.grants().len()))
        .collect();
    owned.sort();
    assert_eq!(
        owned,
        [(vec!["core", "content"], 0), (vec!["web", "sitemap"], 0)]
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

#[test]
fn deleting_content_sources_or_links_is_destructive() {
    for args in [
        ["core", "content", "delete", "c", "--yes"].as_slice(),
        &["core", "content", "delete-source", "s", "--yes"],
        &["core", "content", "link", "delete", "l", "--yes"],
    ] {
        let descriptor = describe(args);
        assert!(descriptor.is_destructive(), "{args:?}");
        assert_eq!(
            descriptor.data_impact(),
            DataImpact::Destructive,
            "{args:?}"
        );
    }
}

#[test]
fn the_read_commands_preserve_data() {
    for args in [
        ["core", "content", "list"].as_slice(),
        &["core", "content", "link", "list"],
        &["core", "content", "analytics", "journey"],
        &["web", "sitemap", "show"],
    ] {
        let descriptor = describe(args);
        assert!(!descriptor.is_destructive(), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
    }
}

#[test]
fn core_content_needs_the_database_and_sitemap_the_profile_paths() {
    assert!(describe(&["core", "content", "list"]).database());
    let sitemap = describe(&["web", "sitemap", "show"]);
    assert!(sitemap.paths(), "web sitemap reads the profile paths");
    assert!(!sitemap.database(), "web sitemap never dials the database");
}
