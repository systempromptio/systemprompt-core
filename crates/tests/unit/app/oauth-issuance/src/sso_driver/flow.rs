use systemprompt_oauth_issuance::sso::{BINDER_COOKIE_NAME, FinishOutcome, SSO_STATE_TTL_SECS};
use systemprompt_test_fixtures::closed_db_pool;
use systemprompt_traits::sso::{CookieSameSite, SsoError};

use super::providers::{
    BLANK, CLOSED, DEFAULT_RETURN, OPEN, OPEN_CALLBACK, Recorded, events_for_sub,
};
use super::{finish, harness, harness_on, refusal_reason, start, success_return_to, unique};

#[tokio::test]
async fn start_binds_the_attempt_to_a_callback_scoped_secure_cookie() {
    let h = harness().await;

    let outcome = h
        .driver
        .start(OPEN, Some("/dashboard"))
        .await
        .expect("start");

    assert!(
        outcome
            .redirect_url
            .contains("redirect_uri=https://sso.test/api/v1/core/sso/test-sso-open/callback"),
        "{}",
        outcome.redirect_url
    );
    let cookie = outcome.binder_cookie;
    assert_eq!(cookie.name, BINDER_COOKIE_NAME);
    assert_eq!(cookie.path, OPEN_CALLBACK);
    assert_eq!(cookie.max_age, SSO_STATE_TTL_SECS);
    assert_eq!(cookie.same_site, CookieSameSite::None);
    assert!(cookie.secure);
    assert!(!cookie.value.is_empty());
}

#[tokio::test]
async fn a_plain_http_deployment_gets_a_lax_insecure_binder() {
    let h = harness_on(
        &systemprompt_test_fixtures::test_db_pool().await,
        "http://sso.test",
    );

    let outcome = h.driver.start(OPEN, None).await.expect("start");

    assert!(!outcome.binder_cookie.secure);
    assert_eq!(outcome.binder_cookie.same_site, CookieSameSite::Lax);
}

#[tokio::test]
async fn start_for_an_unknown_provider_is_unavailable() {
    let h = harness().await;

    let err = h.driver.start("nope", None).await.expect_err("unknown id");

    assert!(matches!(err, SsoError::Unavailable(_)), "{err:?}");
}

#[tokio::test]
async fn a_provider_returning_an_empty_state_cannot_start() {
    let h = harness().await;

    let err = h.driver.start(BLANK, None).await.expect_err("empty state");

    assert!(matches!(err, SsoError::Internal(_)), "{err:?}");
}

#[tokio::test]
async fn start_fails_closed_when_the_state_store_is_down() {
    let h = harness_on(&closed_db_pool().await, "https://sso.test");

    let err = h.driver.start(OPEN, None).await.expect_err("closed pool");

    assert!(matches!(err, SsoError::Internal(_)), "{err:?}");
}

#[tokio::test]
async fn a_full_sign_in_provisions_the_user_and_returns_to_the_requested_page() {
    let h = harness().await;
    let sub = unique("sub");
    let started = start(&h.driver, OPEN, Some("/dashboard")).await;

    let outcome = finish(
        &h.driver,
        OPEN,
        &[
            ("state", &started.state),
            ("sub", &sub),
            ("email", "  Alice@Example.TEST "),
            ("name", "Alice"),
        ],
        Some(&started.binder),
    )
    .await;

    let FinishOutcome::Success {
        access_token,
        max_age_secs,
        return_to,
        clear_binder,
    } = &outcome
    else {
        panic!("expected success, got {outcome:?}");
    };
    assert_eq!(return_to, "/dashboard");
    assert_eq!(access_token.split('.').count(), 3);
    assert_eq!(*max_age_secs, h.driver.config().jwt_access_token_expiration);
    assert_eq!(clear_binder.max_age, 0);
    assert!(clear_binder.value.is_empty());
    assert_eq!(clear_binder.path, OPEN_CALLBACK);
    assert_eq!(h.sessions.created.lock().expect("lock").len(), 1);
    assert_eq!(
        events_for_sub(&sub),
        vec![Recorded::Success {
            sub: sub.clone(),
            email: "alice@example.test".to_owned(),
            created: true,
        }]
    );
}

#[tokio::test]
async fn an_off_origin_return_to_falls_back_to_the_provider_default() {
    let h = harness().await;
    let started = start(&h.driver, OPEN, Some("//evil.test/steal")).await;

    let outcome = finish(
        &h.driver,
        OPEN,
        &[
            ("state", &started.state),
            ("sub", &unique("sub")),
            ("email", "bob@example.test"),
        ],
        Some(&started.binder),
    )
    .await;

    assert_eq!(success_return_to(&outcome), DEFAULT_RETURN);
}

#[tokio::test]
async fn finish_for_an_unknown_provider_refuses_to_the_site_root() {
    let h = harness().await;

    let outcome = finish(&h.driver, "nope", &[], None).await;

    let FinishOutcome::Refused {
        login_path,
        reason,
        clear_binder,
    } = outcome
    else {
        panic!("expected refusal");
    };
    assert_eq!(login_path, "/");
    assert_eq!(reason, "unavailable");
    assert_eq!(clear_binder.path, "/");
    assert_eq!(clear_binder.same_site, CookieSameSite::Lax);
}

#[tokio::test]
async fn a_state_can_be_consumed_only_once() {
    let h = harness().await;
    let started = start(&h.driver, OPEN, None).await;
    let sub = unique("sub");
    let params = [
        ("state", started.state.as_str()),
        ("sub", sub.as_str()),
        ("email", "carol@example.test"),
    ];

    let first = finish(&h.driver, OPEN, &params, Some(&started.binder)).await;
    let second = finish(&h.driver, OPEN, &params, Some(&started.binder)).await;

    success_return_to(&first);
    assert_eq!(refusal_reason(&second), "correlation_lost");
    let FinishOutcome::Refused { login_path, .. } = second else {
        unreachable!()
    };
    assert_eq!(login_path, "/login");
}

#[tokio::test]
async fn a_return_from_another_browser_is_refused_and_audited() {
    let h = harness().await;
    let started = start(&h.driver, OPEN, None).await;
    let sub = unique("sub");

    let outcome = finish(
        &h.driver,
        OPEN,
        &[
            ("state", &started.state),
            ("sub", &sub),
            ("email", "d@example.test"),
        ],
        Some("sso_some_other_binder"),
    )
    .await;

    assert_eq!(refusal_reason(&outcome), "correlation_lost");
    assert!(
        events_for_sub(&sub).is_empty(),
        "correlation fails before the identity is known"
    );
}

#[tokio::test]
async fn a_strict_provider_refuses_an_unsolicited_return() {
    let h = harness().await;

    let outcome = finish(
        &h.driver,
        OPEN,
        &[("sub", &unique("sub")), ("email", "e@example.test")],
        None,
    )
    .await;

    assert_eq!(refusal_reason(&outcome), "correlation_lost");
}

#[tokio::test]
async fn a_strict_provider_refuses_an_unknown_state() {
    let h = harness().await;

    let outcome = finish(
        &h.driver,
        OPEN,
        &[("state", &unique("st")), ("sub", &unique("sub"))],
        Some("sso_binder"),
    )
    .await;

    assert_eq!(refusal_reason(&outcome), "correlation_lost");
}

#[tokio::test]
async fn a_saml_relay_state_correlates_like_an_oidc_state() {
    let h = harness().await;
    let started = start(&h.driver, OPEN, Some("/relay")).await;

    let outcome = finish(
        &h.driver,
        OPEN,
        &[
            ("RelayState", &started.state),
            ("sub", &unique("sub")),
            ("email", "f@example.test"),
        ],
        Some(&started.binder),
    )
    .await;

    assert_eq!(success_return_to(&outcome), "/relay");
}

#[tokio::test]
async fn a_permissive_provider_admits_a_state_without_a_binder() {
    let h = harness().await;
    let sub = unique("sub");
    let user = super::stores::auth_user(&unique("user"), "g@example.test");
    h.users.seed(user.clone());
    h.users.seed_link("https://idp.test/closed", &sub, &user.id);
    let started = start(&h.driver, CLOSED, Some("/after")).await;

    let outcome = finish(
        &h.driver,
        CLOSED,
        &[
            ("state", &started.state),
            ("sub", &sub),
            ("email", "g@example.test"),
        ],
        None,
    )
    .await;

    assert_eq!(success_return_to(&outcome), "/after");
    assert_eq!(
        events_for_sub(&sub),
        vec![Recorded::Success {
            sub,
            email: "g@example.test".to_owned(),
            created: false,
        }]
    );
}

#[tokio::test]
async fn finish_refuses_with_error_when_the_state_store_is_down() {
    let h = harness_on(&closed_db_pool().await, "https://sso.test");

    let outcome = finish(&h.driver, OPEN, &[("state", "st-x")], Some("b")).await;

    assert_eq!(refusal_reason(&outcome), "error");
}
