use std::collections::BTreeMap;

use systemprompt_traits::sso::{ExternalIdentity, SsoIdentityPolicy, SsoRefusal};

fn identity(groups: &[&str]) -> ExternalIdentity {
    ExternalIdentity {
        subject: "sub-1".to_owned(),
        email: Some("Alice@Example.com".to_owned()),
        email_verified: true,
        display_name: None,
        groups: groups.iter().map(|g| (*g).to_owned()).collect(),
        groups_overage: false,
        tenant_id: None,
        assertion_id: None,
        assertion_expires_at: None,
        raw_claims: serde_json::Value::Null,
    }
}

fn roles(map: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    map.iter()
        .map(|(k, v)| ((*k).to_owned(), v.iter().map(|r| (*r).to_owned()).collect()))
        .collect()
}

#[test]
fn default_policy_grants_user_role() {
    let policy = SsoIdentityPolicy::default();
    assert_eq!(policy.default_roles, vec!["user".to_owned()]);
    assert!(!policy.auto_provision);
    let gated = policy
        .gate(&identity(&[]), "alice@example.com")
        .expect("admitted");
    assert_eq!(gated.roles, vec!["user".to_owned()]);
    assert_eq!(gated.display_name, "alice@example.com");
    assert_eq!(gated.external_sub, "sub-1");
}

#[test]
fn deserialised_empty_policy_defaults_roles() {
    let policy: SsoIdentityPolicy = serde_json::from_str("{}").expect("parse");
    assert_eq!(policy.default_roles, vec!["user".to_owned()]);
    assert!(policy.allowed_email_domains.is_empty());
}

#[test]
fn email_domain_match_is_case_insensitive_and_empty_allows_all() {
    let open = SsoIdentityPolicy::default();
    assert!(open.email_allowed("anyone@anywhere.test"));

    let policy = SsoIdentityPolicy {
        allowed_email_domains: vec!["Example.COM".to_owned()],
        ..SsoIdentityPolicy::default()
    };
    assert!(policy.email_allowed("alice@example.com"));
    assert!(!policy.email_allowed("alice@example.com.evil.test"));
    assert!(!policy.email_allowed("no-at-sign"));
}

#[test]
fn gate_checks_domain_before_overage_before_group() {
    let policy = SsoIdentityPolicy {
        allowed_email_domains: vec!["example.com".to_owned()],
        deny_without_group: true,
        ..SsoIdentityPolicy::default()
    };
    let mut overage = identity(&[]);
    overage.groups_overage = true;

    assert_eq!(
        policy.gate(&overage, "alice@other.test"),
        Err(SsoRefusal::DomainNotAllowed)
    );
    assert_eq!(
        policy.gate(&overage, "alice@example.com"),
        Err(SsoRefusal::GroupOverage)
    );
    assert_eq!(
        policy.gate(&identity(&[]), "alice@example.com"),
        Err(SsoRefusal::NoGroup)
    );
}

#[test]
fn roles_union_exact_and_glob_deduplicated_in_order() {
    let policy = SsoIdentityPolicy {
        group_roles: roles(&[("admins", &["admin", "user"]), ("staff", &["user"])]),
        group_role_patterns: roles(&[("team-*", &["editor"]), ("*-ops", &["admin", "ops"])]),
        ..SsoIdentityPolicy::default()
    };
    let groups = vec![
        "staff".to_owned(),
        "admins".to_owned(),
        "team-red".to_owned(),
        "site-ops".to_owned(),
    ];
    assert_eq!(
        policy.roles_for_groups(&groups),
        vec!["user", "admin", "editor", "ops"]
    );
}

#[test]
fn glob_requires_prefix_and_suffix_without_overlap() {
    let policy = SsoIdentityPolicy {
        group_role_patterns: roles(&[("ab*ba", &["x"])]),
        ..SsoIdentityPolicy::default()
    };
    assert!(policy.roles_for_groups(&["aba".to_owned()]).is_empty());
    assert_eq!(policy.roles_for_groups(&["abba".to_owned()]), vec!["x"]);
}

#[test]
fn unmapped_groups_fall_back_to_default_roles() {
    let policy = SsoIdentityPolicy {
        group_roles: roles(&[("admins", &["admin"])]),
        default_roles: vec!["viewer".to_owned()],
        ..SsoIdentityPolicy::default()
    };
    let mut named = identity(&["unrelated"]);
    named.display_name = Some("Alice".to_owned());
    let gated = policy.gate(&named, "alice@example.com").expect("admitted");
    assert_eq!(gated.roles, vec!["viewer"]);
    assert_eq!(gated.display_name, "Alice");
    assert_eq!(gated.groups, vec!["unrelated"]);
}
