//! `admit_extensions` judges an owner's declarations together: every core row
//! of an owner that declares any router is claimed by exactly one of its
//! declarations, so a router declared with a sibling row's shape, a missing
//! router and a duplicated one all refuse boot; and a claimed router is
//! bounded to its row's route pins, so a signed-in route served from the
//! public router, or any route a pinned router adds outside its pins,
//! answers 404. The pins are also checked on the tree the production mount
//! builds (both content routers nested at `/api/v1/content`, the redirect
//! merged at the web root), so a pin layer that saw the unstripped path
//! would fail it.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::routing::{get, post};
use systemprompt_api::services::middleware::{
    CoreDeclaration, MountError, PinnedMethod, RoutePin, admit_extensions, place,
};
use systemprompt_extension::registry::{ApiRouter, WEB_ROOT_BASE_PATH};
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRouter};
use systemprompt_models::modules::ApiPaths;
use tower::ServiceExt;

use super::core_declaration::row;

#[derive(Debug)]
struct Declarer(&'static str);

impl Extension for Declarer {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.0,
            name: self.0,
            version: "0.0.0",
        }
    }
}

fn declared(id: &'static str, router: ExtensionRouter) -> ApiRouter {
    (Arc::new(Declarer(id)), router)
}

fn content(entry: CoreDeclaration) -> ApiRouter {
    declared("content-http", row(entry))
}

fn shaped_as(entry: CoreDeclaration, router: Router) -> ApiRouter {
    let mut declaration = row(entry);
    declaration.router = router;
    declared("content-http", declaration)
}

#[test]
fn the_owners_rows_each_claimed_once_are_admitted() {
    let mut all = vec![
        content(CoreDeclaration::ContentPublic),
        content(CoreDeclaration::ContentAuthenticated),
        content(CoreDeclaration::ContentRedirect),
    ];
    admit_extensions(&mut all).expect("each content row is claimed once");
}

#[test]
fn the_signed_in_router_declared_with_the_public_shape_refuses_boot() {
    let mut swapped = vec![
        content(CoreDeclaration::ContentPublic),
        content(CoreDeclaration::ContentPublic),
        content(CoreDeclaration::ContentRedirect),
    ];
    let err = admit_extensions(&mut swapped).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreDeclarationClaimedTwice {
                extension: "content-http",
                row: CoreDeclaration::ContentPublic,
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_missing_declaration_refuses_boot() {
    let mut missing = vec![
        content(CoreDeclaration::ContentPublic),
        content(CoreDeclaration::ContentRedirect),
    ];
    let err = admit_extensions(&mut missing).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreDeclarationUnclaimed {
                extension: "content-http",
                row: CoreDeclaration::ContentAuthenticated,
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_duplicated_declaration_refuses_boot() {
    let mut duplicated = vec![
        content(CoreDeclaration::ContentPublic),
        content(CoreDeclaration::ContentAuthenticated),
        content(CoreDeclaration::ContentAuthenticated),
        content(CoreDeclaration::ContentRedirect),
    ];
    let err = admit_extensions(&mut duplicated).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreDeclarationClaimedTwice {
                extension: "content-http",
                row: CoreDeclaration::ContentAuthenticated,
            }
        ),
        "{err:?}"
    );
}

#[test]
fn an_owner_that_declares_nothing_claims_nothing() {
    let mut others = vec![declared("install", row(CoreDeclaration::ContentRedirect))];
    admit_extensions(&mut others).expect("a disabled owner leaves its rows unclaimed");
    admit_extensions(&mut []).expect("no declarations");
}

#[test]
fn the_engagement_row_is_the_only_admitted_web_path() {
    let mut analytics = vec![
        declared("analytics-http", row(CoreDeclaration::Analytics)),
        declared("analytics-http", row(CoreDeclaration::Engagement)),
    ];
    admit_extensions(&mut analytics).expect("engagement holds the WebPath allowance");

    let mut install = vec![declared("install", row(CoreDeclaration::Engagement))];
    let err = admit_extensions(&mut install).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::ExtensionWebPath {
                extension: "install",
                base_path: "/track/engagement",
            }
        ),
        "{err:?}"
    );
}

#[test]
fn only_the_signed_in_and_redirect_rows_pin_routes() {
    let pinned: Vec<(CoreDeclaration, &[RoutePin])> = CoreDeclaration::ALL
        .iter()
        .map(|entry| (*entry, entry.route_pins()))
        .filter(|(_, pins)| !pins.is_empty())
        .collect();
    assert_eq!(
        pinned,
        vec![
            (
                CoreDeclaration::ContentAuthenticated,
                &[RoutePin::new(PinnedMethod::Post, "/links/generate")][..],
            ),
            (
                CoreDeclaration::ContentRedirect,
                &[RoutePin::new(PinnedMethod::Get, "/r")][..],
            ),
        ]
    );
}

async fn status(router: &Router, method: Method, uri: &str) -> StatusCode {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("infallible")
        .status()
}

fn admitted_routers(public: Router, signed_in: Router, redirect: Router) -> Vec<Router> {
    let mut all = vec![
        shaped_as(CoreDeclaration::ContentPublic, public),
        shaped_as(CoreDeclaration::ContentAuthenticated, signed_in),
        shaped_as(CoreDeclaration::ContentRedirect, redirect),
    ];
    admit_extensions(&mut all).expect("admitted");
    all.into_iter()
        .map(|(_, declared)| declared.router)
        .collect()
}

#[tokio::test]
async fn each_content_router_answers_only_its_own_rows_routes() {
    let routers = admitted_routers(
        Router::new()
            .route("/links", get(|| async { "links" }))
            .route("/links/journey", get(|| async { "journey" }))
            .route("/links/generate", post(|| async { "moved into public" })),
        Router::new()
            .route("/links/generate", post(|| async { "generated" }))
            .route("/links/other", post(|| async { "outside the pins" }))
            .route("/links/generate/extra", get(|| async { "wrong method" })),
        Router::new()
            .route("/r/{code}", get(|| async { "redirect" }))
            .route("/oauth/escape", get(|| async { "outside the pins" })),
    );
    let [public, signed_in, redirect] = routers.as_slice() else {
        panic!("three routers");
    };

    assert_eq!(status(public, Method::GET, "/links").await, StatusCode::OK);
    assert_eq!(
        status(public, Method::GET, "/links/journey").await,
        StatusCode::OK
    );
    assert_eq!(
        status(public, Method::POST, "/links/generate").await,
        StatusCode::NOT_FOUND,
        "a signed-in route is not served from the public declaration"
    );

    assert_eq!(
        status(signed_in, Method::POST, "/links/generate").await,
        StatusCode::OK
    );
    assert_eq!(
        status(signed_in, Method::POST, "/links/other").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(signed_in, Method::GET, "/links/generate/extra").await,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        status(redirect, Method::GET, "/r/abc").await,
        StatusCode::OK
    );
    assert_eq!(
        status(redirect, Method::GET, "/oauth/escape").await,
        StatusCode::NOT_FOUND
    );
}

async fn answer(router: &Router, method: Method, uri: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("infallible");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_pins_hold_on_the_tree_the_production_mount_builds() {
    let routers = admitted_routers(
        Router::new()
            .route("/links", get(|| async { "public links" }))
            .route("/links/journey", get(|| async { "public journey" }))
            .route("/links/{link_id}", get(|| async { "public link" })),
        Router::new()
            .route("/links/generate", post(|| async { "signed-in generate" }))
            .route(
                "/links/other",
                post(|| async { "signed-in outside the pins" }),
            ),
        Router::new()
            .route("/r/{code}", get(|| async { "redirect" }))
            .route(
                "/oauth/escape",
                get(|| async { "redirect outside the pins" }),
            ),
    );
    let [public, signed_in, redirect] = <[Router; 3]>::try_from(routers).expect("three routers");
    let tree = place(Router::new(), ApiPaths::CONTENT_BASE, public);
    let tree = place(tree, ApiPaths::CONTENT_BASE, signed_in);
    let tree = place(tree, WEB_ROOT_BASE_PATH, redirect);

    assert_eq!(
        answer(&tree, Method::POST, "/api/v1/content/links/generate").await,
        (StatusCode::OK, "signed-in generate".to_owned()),
        "the signed-in pin matches the path relative to the nest"
    );
    assert_eq!(
        answer(&tree, Method::GET, "/api/v1/content/links").await,
        (StatusCode::OK, "public links".to_owned())
    );
    assert_eq!(
        answer(&tree, Method::GET, "/api/v1/content/links/journey").await,
        (StatusCode::OK, "public journey".to_owned())
    );
    assert_eq!(
        answer(&tree, Method::GET, "/api/v1/content/links/abc").await,
        (StatusCode::OK, "public link".to_owned())
    );
    let (status, body) = answer(&tree, Method::GET, "/api/v1/content/links/generate").await;
    assert_ne!(status, StatusCode::OK, "{body}");
    assert_ne!(body, "public link");
    assert_eq!(
        answer(&tree, Method::POST, "/api/v1/content/links/other")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        answer(&tree, Method::GET, "/r/x").await,
        (StatusCode::OK, "redirect".to_owned())
    );
    assert_eq!(
        answer(&tree, Method::GET, "/oauth/escape").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        answer(&tree, Method::GET, "/api/v1/content/r/x").await.0,
        StatusCode::NOT_FOUND
    );
}
