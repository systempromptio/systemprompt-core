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
    use chrono::{Duration, Utc};
    use systemprompt_api::services::middleware::{SiteAuthSession, site_auth_gate};
    use systemprompt_api::services::session_cookie::SessionCookie;
    use systemprompt_database::DbPool;
    use systemprompt_extension::SiteAuthConfig;
    use systemprompt_identifiers::{AccessTokenId, UserId};
    use systemprompt_models::auth::{JwtAudience, Permission};
    use systemprompt_test_fixtures::{
        fixture_request_authenticator, seed_user_row_with_roles, session_cookie_with, test_db_pool,
    };
    use tower::ServiceExt;

    const CONFIG: SiteAuthConfig = SiteAuthConfig {
        login_path: "/admin/login",
        protected_prefixes: &["/admin"],
        public_prefixes: &["/public"],
        required_scope: "admin",
    };

    const USER_SITE: SiteAuthConfig = SiteAuthConfig {
        login_path: "/app/login",
        protected_prefixes: &["/app"],
        public_prefixes: &[],
        required_scope: "user",
    };

    struct Signed {
        pool: DbPool,
        user: UserId,
        cookie: String,
        jti: AccessTokenId,
    }

    fn app(config: SiteAuthConfig, pool: &DbPool) -> Router {
        let authenticator = fixture_request_authenticator(pool);
        Router::new()
            .fallback(get(|| async { "handler-reached" }))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                let authenticator = authenticator.clone();
                async move {
                    site_auth_gate(
                        req,
                        next,
                        config,
                        SiteAuthSession {
                            authenticator: &authenticator,
                            cookie: SessionCookie::for_external_url("https://app.example.com"),
                        },
                    )
                    .await
                }
            }))
    }

    async fn signed_in_with(
        roles: &[&str],
        edit: impl FnOnce(&mut systemprompt_models::auth::UserJwtRequest),
    ) -> Signed {
        let pool = test_db_pool().await;
        let user = UserId::new(uuid::Uuid::new_v4().to_string());
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        seed_user_row_with_roles(&pool, &user, &format!("{user}@site-auth.invalid"), &roles)
            .await
            .expect("seed user");
        let jti = AccessTokenId::generate();
        let minted = jti.clone();
        let cookie = session_cookie_with(&pool, &user, 0, move |request| {
            request.jti = minted;
            request.permissions = vec![Permission::Admin];
            edit(request);
        })
        .await
        .expect("mint session cookie");
        Signed {
            pool,
            user,
            cookie,
            jti,
        }
    }

    async fn signed_in(roles: &[&str]) -> Signed {
        signed_in_with(roles, |_| {}).await
    }

    async fn send(
        config: SiteAuthConfig,
        pool: &DbPool,
        uri: &str,
        cookie: Option<&str>,
    ) -> axum::http::Response<Body> {
        let mut builder = Request::builder().uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        app(config, pool)
            .oneshot(builder.body(Body::empty()).expect("request must build"))
            .await
            .expect("request must complete")
    }

    async fn request(uri: &str, cookie: Option<&str>) -> axum::http::Response<Body> {
        send(CONFIG, &test_db_pool().await, uri, cookie).await
    }

    fn redirected_and_cleared(resp: &axum::http::Response<Body>) -> bool {
        resp.status() == StatusCode::SEE_OTHER
            && resp
                .headers()
                .get(header::SET_COOKIE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|cookie| cookie.contains("access_token=;"))
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
        let resp = request("/admin/agents", Some("access_token=not-a-jwt")).await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(cookie.contains("access_token=;"), "{cookie}");
        assert!(cookie.contains("Max-Age=0"), "{cookie}");
        assert!(cookie.contains("Secure"), "{cookie}");
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
    async fn a_live_admin_session_reaches_the_handler() {
        let admin = signed_in(&["admin", "user"]).await;
        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "an admin must reach an admin page"
        );
    }

    #[tokio::test]
    async fn a_logged_out_admins_unexpired_token_is_refused() {
        let admin = signed_in(&["admin", "user"]).await;
        sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1")
            .bind(admin.user.as_str())
            .execute(admin.pool.write_pool().as_ref())
            .await
            .expect("end the session");

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert!(redirected_and_cleared(&resp), "{:?}", resp.status());
    }

    #[tokio::test]
    async fn a_revoked_admin_token_is_refused() {
        let admin = signed_in(&["admin", "user"]).await;
        systemprompt_test_fixtures::oauth_fixture_repository(&admin.pool)
            .revoke_jti(&admin.jti, &admin.user, Utc::now() + Duration::hours(1))
            .await
            .expect("revoke the token id");

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert!(redirected_and_cleared(&resp), "{:?}", resp.status());
    }

    #[tokio::test]
    async fn a_demoted_admins_admin_scoped_token_is_refused_the_admin_site() {
        let admin = signed_in(&["admin", "user"]).await;
        sqlx::query("UPDATE users SET roles = ARRAY['user'] WHERE id = $1")
            .bind(admin.user.as_str())
            .execute(admin.pool.write_pool().as_ref())
            .await
            .expect("demote the user");

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert_eq!(
            resp.status(),
            StatusCode::SEE_OTHER,
            "the role is the user row's, never the token's scope"
        );
    }

    #[tokio::test]
    async fn a_deleted_users_token_is_refused() {
        let admin = signed_in(&["admin", "user"]).await;
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(admin.user.as_str())
            .execute(admin.pool.write_pool().as_ref())
            .await
            .expect("delete the user");

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert!(redirected_and_cleared(&resp), "{:?}", resp.status());
    }

    #[tokio::test]
    async fn a_token_of_another_audience_is_refused() {
        let admin = signed_in_with(&["admin", "user"], |request| {
            request.audience = vec![JwtAudience::Mcp];
        })
        .await;

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert!(redirected_and_cleared(&resp), "{:?}", resp.status());
    }

    #[tokio::test]
    async fn a_session_naming_another_user_is_refused() {
        let admin = signed_in(&["admin", "user"]).await;
        let other = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row_with_roles(
            &admin.pool,
            &other,
            &format!("{other}@site-auth.invalid"),
            &[],
        )
        .await
        .expect("seed the other user");
        sqlx::query("UPDATE user_sessions SET user_id = $1 WHERE user_id = $2")
            .bind(other.as_str())
            .bind(admin.user.as_str())
            .execute(admin.pool.write_pool().as_ref())
            .await
            .expect("rebind the session");

        let resp = send(CONFIG, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

        assert!(redirected_and_cleared(&resp), "{:?}", resp.status());
    }

    #[tokio::test]
    async fn an_unparseable_required_scope_denies_rather_than_admits() {
        const BROKEN: SiteAuthConfig = SiteAuthConfig {
            login_path: "/admin/login",
            protected_prefixes: &["/admin"],
            public_prefixes: &[],
            required_scope: "not-a-real-permission",
        };
        let admin = signed_in(&["admin", "user"]).await;

        let resp = send(BROKEN, &admin.pool, "/admin/agents", Some(&admin.cookie)).await;

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

        let resp = send(GATE_ALL, &test_db_pool().await, "/anything-at-all", None).await;

        assert_eq!(
            resp.status(),
            StatusCode::SEE_OTHER,
            "declaring no protected prefixes must mean everything is protected"
        );
    }

    #[tokio::test]
    async fn a_user_scoped_site_admits_user_and_admin_rows() {
        for roles in [&["user"][..], &["admin", "user"][..]] {
            let signed = signed_in(roles).await;
            let resp = send(USER_SITE, &signed.pool, "/app/home", Some(&signed.cookie)).await;
            assert_eq!(resp.status(), StatusCode::OK, "{roles:?}");
        }
    }

    #[tokio::test]
    async fn an_admin_scoped_site_refuses_a_user_row_whatever_the_token_scope() {
        let user = signed_in(&["user"]).await;
        let resp = send(CONFIG, &user.pool, "/admin/agents", Some(&user.cookie)).await;
        assert_ne!(resp.status(), StatusCode::OK);
    }


    #[tokio::test]
    async fn a_token_without_a_jti_is_refused() {
        use systemprompt_models::auth::{
            CustomClaims, JwtClaims, RateLimitTier, RegisteredClaims, TokenType, UserType,
        };
        let pool = test_db_pool().await;
        let user = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row_with_roles(
            &pool,
            &user,
            &format!("{user}@site-auth.invalid"),
            &["admin".to_owned(), "user".to_owned()],
        )
        .await
        .expect("seed user");
        let session = systemprompt_identifiers::SessionId::generate();
        systemprompt_test_fixtures::seed_user_session(&pool, &user, &session)
            .await
            .expect("seed session");
        let now = Utc::now();
        let claims = JwtClaims {
            registered: RegisteredClaims {
                sub: user.to_string(),
                iat: now.timestamp(),
                exp: (now + Duration::hours(1)).timestamp(),
                nbf: Some(now.timestamp()),
                iss: systemprompt_test_fixtures::FIXTURE_JWT_ISSUER.to_owned(),
                aud: JwtAudience::standard(),
                jti: String::new(),
            },
            custom: CustomClaims {
                scope: vec![Permission::Admin],
                username: user.to_string(),
                email: format!("{user}@site-auth.invalid"),
                user_type: UserType::Admin,
                roles: vec!["admin".to_owned()],
                attributes: std::collections::BTreeMap::new(),
                client_id: None,
                token_type: TokenType::Bearer,
                auth_time: now.timestamp(),
                session_id: Some(session),
                rate_limit_tier: Some(RateLimitTier::Admin),
                plugin_id: None,
                act: None,
                purpose: None,
                interaction: None,
                browser_session: true,
                ui_session: false,
            },
        };
        let keys = systemprompt_test_fixtures::test_signing_keys();
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(keys.active_kid().to_owned());
        let token =
            jsonwebtoken::encode(&header, &claims, keys.encoding_key()).expect("encode the token");

        let resp = send(
            CONFIG,
            &pool,
            "/admin/agents",
            Some(&format!("access_token={token}")),
        )
        .await;

        assert!(
            redirected_and_cleared(&resp),
            "a token revocation cannot be checked for is refused: {:?}",
            resp.status()
        );
    }
}
