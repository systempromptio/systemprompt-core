//! Tests for the link policy an SSO provider hands the federated resolution,
//! the refusal codes a resolution refusal surfaces as, and the default role
//! resolver.

use systemprompt_identifiers::UserId;
use systemprompt_oauth_issuance::sso::{DirectoryOnlyRoleResolver, RoleResolver};
use systemprompt_traits::sso::{SsoError, SsoIdentityPolicy, SsoRefusal};
use systemprompt_traits::{FederatedRefusal, LinkPolicy};

#[test]
fn the_default_provider_policy_neither_links_nor_provisions() {
    assert_eq!(
        SsoIdentityPolicy::default().link_policy(),
        LinkPolicy {
            link_by_email: false,
            auto_provision: false,
        }
    );
}

#[test]
fn the_provider_policy_carries_both_switches_into_the_link_policy() {
    let policy = SsoIdentityPolicy {
        auto_provision: true,
        link_by_email: true,
        ..SsoIdentityPolicy::default()
    };
    assert_eq!(
        policy.link_policy(),
        LinkPolicy {
            link_by_email: true,
            auto_provision: true,
        }
    );
}

#[test]
fn each_resolution_refusal_has_its_own_reason_code() {
    let codes: Vec<&str> = [
        FederatedRefusal::EmailInUse,
        FederatedRefusal::TargetIneligible,
        FederatedRefusal::NotProvisioned,
    ]
    .into_iter()
    .map(|refusal| SsoError::Refused(SsoRefusal::from(refusal)).reason_code())
    .collect();
    assert_eq!(
        codes,
        ["email_in_use", "link_ineligible", "not_provisioned"]
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
