//! Tests for the SSO user-resolution order and the default role resolver.

use systemprompt_identifiers::UserId;
use systemprompt_oauth_issuance::sso::{
    DirectoryOnlyRoleResolver, ResolutionPlan, RoleResolver, plan_resolution,
};
use systemprompt_traits::AuthUser;
use systemprompt_traits::sso::SsoIdentityPolicy;

fn policy(auto_provision: bool, link_by_email: bool) -> SsoIdentityPolicy {
    SsoIdentityPolicy {
        auto_provision,
        link_by_email,
        ..SsoIdentityPolicy::default()
    }
}

fn owner(id: &str, is_active: bool, is_service_account: bool) -> AuthUser {
    AuthUser {
        id: UserId::new(id),
        name: "operator".to_owned(),
        email: "op@example.com".to_owned(),
        roles: vec!["admin".to_owned()],
        is_active,
        is_service_account,
    }
}

#[test]
fn a_federated_link_wins_over_everything() {
    let linked = UserId::new("linked");
    let other = owner("other", true, false);
    assert_eq!(
        plan_resolution(
            Some(linked.clone()),
            Some(&other),
            true,
            &policy(true, true)
        ),
        ResolutionPlan::Existing(linked)
    );
}

#[test]
fn link_by_email_claims_the_active_interactive_owner_before_provisioning() {
    let op = owner("op", true, false);
    assert_eq!(
        plan_resolution(None, Some(&op), true, &policy(true, true)),
        ResolutionPlan::LinkExisting(UserId::new("op"))
    );
}

#[test]
fn link_by_email_is_off_by_default() {
    let op = owner("op", true, false);
    assert!(!SsoIdentityPolicy::default().link_by_email);
    assert_eq!(
        plan_resolution(None, Some(&op), true, &policy(true, false)),
        ResolutionPlan::Provision
    );
}

#[test]
fn an_unverified_email_never_links() {
    let op = owner("op", true, false);
    assert_eq!(
        plan_resolution(None, Some(&op), false, &policy(false, true)),
        ResolutionPlan::Refuse
    );
}

#[test]
fn inactive_or_service_owners_fall_through() {
    for candidate in [owner("off", false, false), owner("svc", true, true)] {
        assert_eq!(
            plan_resolution(None, Some(&candidate), true, &policy(true, true)),
            ResolutionPlan::Provision
        );
        assert_eq!(
            plan_resolution(None, Some(&candidate), true, &policy(false, true)),
            ResolutionPlan::Refuse
        );
    }
}

#[test]
fn no_owner_and_no_provisioning_refuses() {
    assert_eq!(
        plan_resolution(None, None, true, &policy(false, true)),
        ResolutionPlan::Refuse
    );
}

#[tokio::test]
async fn the_default_role_resolver_returns_the_directory_roles() {
    let roles = vec!["user".to_owned(), "editor".to_owned()];
    let resolved = DirectoryOnlyRoleResolver
        .resolve(&UserId::new("u"), &roles)
        .await
        .expect("resolve");
    assert_eq!(resolved, roles);
}
