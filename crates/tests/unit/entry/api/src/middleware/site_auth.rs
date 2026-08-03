//! Post-login redirect builder for the site-auth gate.

use axum::http::Uri;
use systemprompt_api::services::middleware::login_redirect;

#[test]
fn preserves_query_string() {
    let uri: Uri = "/bridge-auth/device-link?redirect=http://127.0.0.1:9000/cb"
        .parse()
        .unwrap();
    let out = login_redirect("/admin/login", &uri);
    assert_eq!(
        out,
        "/admin/login?redirect=%2Fbridge-auth%2Fdevice-link%3Fredirect%3Dhttp%3A%2F%2F127.0.0.1%3A9000%2Fcb"
    );
}

#[test]
fn path_only_round_trips() {
    let uri: Uri = "/admin/agents".parse().unwrap();
    let out = login_redirect("/admin/login", &uri);
    assert_eq!(out, "/admin/login?redirect=%2Fadmin%2Fagents");
}

#[test]
fn nested_redirect_is_fully_encoded() {
    let uri: Uri = "/bridge-auth/device-link?redirect=x&evil=1"
        .parse()
        .unwrap();
    let out = login_redirect("/admin/login", &uri);
    // The whole original target is one encoded value — its `&` is escaped, so it
    // cannot inject a sibling query param into the login URL.
    assert!(!out.contains("&evil"));
    assert!(out.contains("%26evil%3D1"));
}

mod gate {
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{StatusCode, header};
    use axum::routing::get;
    use axum::{Router, middleware};
    use systemprompt_api::services::middleware::site_auth_gate;
    use systemprompt_extension::SiteAuthConfig;
    use systemprompt_identifiers::UserId;
    use systemprompt_test_fixtures::{fixture_config, install_test_signing_key, mint_admin_jwt};
    use tower::ServiceExt;

    const CONFIG: SiteAuthConfig = SiteAuthConfig {
        login_path: "/admin/login",
        protected_prefixes: &["/admin"],
        public_prefixes: &["/public"],
        required_scope: "admin",
    };

    fn app(config: SiteAuthConfig) -> Router {
        Router::new()
            .fallback(get(|| async { "handler-reached" }))
            .layer(middleware::from_fn(move |req, next| {
                site_auth_gate(req, next, config.clone())
            }))
    }

    async fn request(uri: &str, cookie: Option<&str>) -> axum::http::Response<Body> {
        let mut builder = Request::builder().uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, format!("access_token={cookie}"));
        }
        app(CONFIG)
            .oneshot(builder.body(Body::empty()).expect("request must build"))
            .await
            .expect("request must complete")
    }

    fn admin_token() -> String {
        install_test_signing_key();
        let config = fixture_config("postgres://unused/unused");
        mint_admin_jwt(
            &UserId::new("site-auth-admin"),
            "site-auth@example.invalid",
            &config.jwt_issuer,
        )
        .as_str()
        .to_owned()
    }

    #[tokio::test]
    async fn the_login_page_itself_is_always_reachable() {
        assert_eq!(request("/admin/login", None).await.status(), StatusCode::OK);
        assert_eq!(
            request("/admin/login/", None).await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_public_prefix_bypasses_the_gate() {
        assert_eq!(
            request("/public/pricing", None).await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn a_path_outside_the_protected_prefixes_is_not_gated() {
        assert_eq!(
            request("/blog/some-post", None).await.status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn static_assets_are_served_without_a_session() {
        for asset in ["/admin/app.js", "/admin/style.css", "/admin/logo.svg"] {
            assert_eq!(
                request(asset, None).await.status(),
                StatusCode::OK,
                "{asset} must not be gated, or the login page cannot render"
            );
        }
    }

    #[tokio::test]
    async fn an_unauthenticated_protected_page_redirects_to_login() {
        let resp = request("/admin/agents", None).await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            location.starts_with("/admin/login?redirect="),
            "the gate must send the browser back after login: {location}"
        );
    }

    #[tokio::test]
    async fn an_unparseable_token_is_cleared_on_the_way_to_login() {
        let resp = request("/admin/agents", Some("not-a-jwt")).await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        // Without this the browser bounces between login and the protected
        // page forever, re-presenting the same unusable token.
        assert!(cookie.contains("access_token=;"), "{cookie}");
        assert!(cookie.contains("Max-Age=0"), "{cookie}");
    }

    #[tokio::test]
    async fn an_absent_token_is_not_cleared_because_there_is_nothing_to_clear() {
        let resp = request("/admin/agents", None).await;

        assert!(
            resp.headers().get(header::SET_COOKIE).is_none(),
            "an unauthenticated request carries no stale cookie to purge"
        );
    }

    #[tokio::test]
    async fn a_valid_admin_token_reaches_the_handler() {
        let resp = request("/admin/agents", Some(&admin_token())).await;

        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "an admin must reach an admin page"
        );
    }

    #[tokio::test]
    async fn an_unparseable_required_scope_denies_rather_than_admits() {
        const BROKEN: SiteAuthConfig = SiteAuthConfig {
            login_path: "/admin/login",
            protected_prefixes: &["/admin"],
            public_prefixes: &[],
            required_scope: "not-a-real-permission",
        };
        let token = admin_token();

        let resp = app(BROKEN)
            .oneshot(
                Request::builder()
                    .uri("/admin/agents")
                    .header(header::COOKIE, format!("access_token={token}"))
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("request must complete");

        // A misconfigured scope must fail closed: admitting everyone would turn
        // a config typo into an open admin panel.
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn an_empty_protected_prefix_list_gates_everything() {
        const GATE_ALL: SiteAuthConfig = SiteAuthConfig {
            login_path: "/admin/login",
            protected_prefixes: &[],
            public_prefixes: &[],
            required_scope: "admin",
        };

        let resp = app(GATE_ALL)
            .oneshot(
                Request::builder()
                    .uri("/anything-at-all")
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("request must complete");

        assert_eq!(
            resp.status(),
            StatusCode::SEE_OTHER,
            "declaring no protected prefixes must mean everything is protected"
        );
    }
}
