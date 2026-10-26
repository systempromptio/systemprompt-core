use systemprompt_identifiers::UserId;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_oauth_issuance::sso::{
    BrowserSessionSubject, FinishOutcome, mint_browser_session,
};
use systemprompt_traits::SessionAnalytics;
use systemprompt_traits::sso::SsoError;

use super::providers::{
    CLOSED, METADATA_BODY, OPEN, RESOLVED_ROLE, Recorded, events_for_sub, refusals_with_reason,
};
use super::stores::auth_user;
use super::{Harness, finish, harness, refusal_reason, start, success_return_to, unique};

const OPEN_ISSUER: &str = "https://idp.test/open";

async fn sign_in(h: &Harness, params: &[(&str, &str)]) -> FinishOutcome {
    let started = start(&h.driver, OPEN, None).await;
    let mut all = vec![("state", started.state.as_str())];
    all.extend_from_slice(params);
    finish(&h.driver, OPEN, &all, Some(&started.binder)).await
}

#[tokio::test]
async fn a_blank_subject_is_refused_before_any_lookup() {
    let h = harness().await;

    let outcome = sign_in(&h, &[("sub", "   "), ("email", "a@example.test")]).await;

    assert_eq!(refusal_reason(&outcome), "no_subject");
    assert_eq!(h.users.link_count(), 0);
}

#[tokio::test]
async fn an_identity_without_email_is_refused_and_audited_with_its_subject() {
    let h = harness().await;
    let sub = unique("sub");

    let outcome = sign_in(&h, &[("sub", &sub), ("email", "  ")]).await;

    assert_eq!(refusal_reason(&outcome), "no_email");
    assert_eq!(
        events_for_sub(&sub),
        vec![Recorded::Refused {
            sub: Some(sub.clone()),
            email: None,
            reason: SsoError::NoEmail.audit_reason(),
        }]
    );
}

#[tokio::test]
async fn a_replayed_assertion_is_refused_on_its_second_use() {
    let h = harness().await;
    let assertion = unique("assertion");
    let params = [
        ("sub", "replay-sub"),
        ("email", "r@example.test"),
        ("assertion", assertion.as_str()),
    ];

    let first = sign_in(&h, &params).await;
    let second = sign_in(&h, &params).await;

    success_return_to(&first);
    assert_eq!(refusal_reason(&second), "invalid_assertion");
}

#[tokio::test]
async fn an_email_outside_the_allowed_domains_is_forbidden() {
    let h = harness().await;
    let sub = unique("sub");

    let outcome = sign_in(&h, &[("sub", &sub), ("email", "m@elsewhere.test")]).await;

    assert_eq!(refusal_reason(&outcome), "forbidden");
    assert!(matches!(
        events_for_sub(&sub).as_slice(),
        [Recorded::Refused { email: Some(e), .. }] if e == "m@elsewhere.test"
    ));
}

#[tokio::test]
async fn a_group_overage_is_refused() {
    let h = harness().await;

    let outcome = sign_in(
        &h,
        &[
            ("sub", &unique("sub")),
            ("email", "o@example.test"),
            ("overage", "1"),
        ],
    )
    .await;

    assert_eq!(refusal_reason(&outcome), "group_overage");
}

#[tokio::test]
async fn a_provider_rejection_is_an_invalid_assertion() {
    let h = harness().await;
    let before = refusals_with_reason(SsoError::InvalidAssertion(String::new()).audit_reason());

    let outcome = sign_in(&h, &[("fail", "1")]).await;

    assert_eq!(refusal_reason(&outcome), "invalid_assertion");
    assert!(
        refusals_with_reason(SsoError::InvalidAssertion(String::new()).audit_reason()) > before
    );
}

#[tokio::test]
async fn directory_groups_map_to_roles_and_the_resolver_has_the_last_word() {
    let h = harness().await;
    let sub = unique("sub");

    let outcome = sign_in(
        &h,
        &[
            ("sub", &sub),
            ("email", "admin@example.test"),
            ("groups", "admins"),
        ],
    )
    .await;

    success_return_to(&outcome);
    let user_id = h.users.federated.lock().expect("lock")[&(OPEN_ISSUER.to_owned(), sub)].clone();
    assert_eq!(
        h.users.roles_of(&user_id),
        Some(vec!["admin".to_owned(), RESOLVED_ROLE.to_owned()])
    );
}

#[tokio::test]
async fn a_verified_email_links_the_existing_account_instead_of_provisioning() {
    let h = harness().await;
    let existing = auth_user(&unique("user"), "link@example.test");
    h.users.seed(existing.clone());
    let sub = unique("sub");

    let outcome = sign_in(
        &h,
        &[
            ("sub", &sub),
            ("email", "link@example.test"),
            ("verified", "1"),
        ],
    )
    .await;

    success_return_to(&outcome);
    assert_eq!(h.users.link_count(), 1);
    assert_eq!(
        events_for_sub(&sub),
        vec![Recorded::Success {
            sub,
            email: "link@example.test".to_owned(),
            created: false,
        }]
    );
    assert_eq!(h.users.users.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn an_unverified_email_never_claims_an_existing_account() {
    let h = harness().await;
    h.users
        .seed(auth_user(&unique("user"), "claim@example.test"));

    let outcome = sign_in(
        &h,
        &[("sub", &unique("sub")), ("email", "claim@example.test")],
    )
    .await;

    success_return_to(&outcome);
    assert_eq!(h.users.link_count(), 0);
    assert_eq!(h.users.users.lock().expect("lock").len(), 2);
}

#[tokio::test]
async fn a_service_account_owning_the_email_is_not_linked() {
    let h = harness().await;
    let mut service = auth_user(&unique("svc"), "svc@example.test");
    service.is_service_account = true;
    h.users.seed(service);

    let outcome = sign_in(
        &h,
        &[
            ("sub", &unique("sub")),
            ("email", "svc@example.test"),
            ("verified", "1"),
        ],
    )
    .await;

    success_return_to(&outcome);
    assert_eq!(h.users.link_count(), 0);
}

#[tokio::test]
async fn a_federated_link_to_a_deactivated_user_is_not_provisioned() {
    let h = harness().await;
    let sub = unique("sub");
    let mut user = auth_user(&unique("user"), "gone@example.test");
    user.is_active = false;
    h.users.seed(user.clone());
    h.users.seed_link(OPEN_ISSUER, &sub, &user.id);

    let outcome = sign_in(&h, &[("sub", &sub), ("email", "gone@example.test")]).await;

    assert_eq!(refusal_reason(&outcome), "not_provisioned");
    assert!(h.sessions.created.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn a_federated_link_to_a_missing_user_is_an_internal_error() {
    let h = harness().await;
    let sub = unique("sub");
    h.users
        .seed_link(OPEN_ISSUER, &sub, &UserId::new(unique("ghost")));

    let outcome = sign_in(&h, &[("sub", &sub), ("email", "ghost@example.test")]).await;

    assert_eq!(refusal_reason(&outcome), "error");
}

#[tokio::test]
async fn a_provider_without_auto_provision_refuses_a_stranger() {
    let h = harness().await;
    let sub = unique("sub");

    let outcome = finish(
        &h.driver,
        CLOSED,
        &[("sub", &sub), ("email", "stranger@example.test")],
        None,
    )
    .await;

    assert_eq!(refusal_reason(&outcome), "not_provisioned");
    let FinishOutcome::Refused { login_path, .. } = &outcome else {
        unreachable!()
    };
    assert_eq!(login_path, "/login");
    assert!(matches!(
        events_for_sub(&sub).as_slice(),
        [Recorded::Refused { email: Some(_), .. }]
    ));
}

#[tokio::test]
async fn minting_a_session_for_an_unknown_user_fails() {
    let h = harness().await;
    let service = SessionCreationService::new(
        std::sync::Arc::clone(&h.sessions)
            as std::sync::Arc<dyn systemprompt_traits::SessionProvider>,
        std::sync::Arc::clone(&h.users) as std::sync::Arc<dyn systemprompt_traits::UserProvider>,
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        ),
        systemprompt_test_fixtures::test_config(),
    );
    let user_id = UserId::new(unique("nobody"));

    let err = mint_browser_session(
        &service,
        h.driver.config(),
        &BrowserSessionSubject {
            user_id: &user_id,
            display_name: "Nobody",
            email: "nobody@example.test",
            roles: &["admin".to_owned()],
        },
        &SessionAnalytics::default(),
    )
    .await
    .expect_err("no such user");

    assert!(matches!(err, SsoError::Internal(_)), "{err:?}");
}

#[tokio::test]
async fn provider_listing_keeps_one_of_a_duplicated_id_and_skips_disabled_ones() {
    let h = harness().await;

    let ids = h.driver.provider_ids();

    assert_eq!(ids.iter().filter(|id| *id == OPEN).count(), 1, "{ids:?}");
    assert!(ids.iter().any(|id| id == CLOSED));
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(format!("{:?}", h.driver).contains(OPEN));
}

#[tokio::test]
async fn metadata_is_served_only_by_providers_that_publish_it() {
    let h = harness().await;

    let doc = h
        .driver
        .metadata(OPEN)
        .await
        .expect("open publishes metadata")
        .expect("metadata builds");

    assert_eq!(doc.body, METADATA_BODY);
    assert!(h.driver.metadata(CLOSED).await.is_none());
    assert!(h.driver.metadata("nope").await.is_none());
}
