// Pure-logic tests for OAuthRepository scope helpers (no DB).

use systemprompt_oauth::repository::OAuthRepository;

#[test]
fn validate_scopes_empty_returns_empty() {
    let out = OAuthRepository::validate_scopes(&[]).expect("empty ok");
    assert!(out.is_empty());
}

#[test]
fn validate_scopes_all_valid() {
    let req = vec![
        "user".to_owned(),
        "admin".to_owned(),
        "anonymous".to_owned(),
    ];
    let out = OAuthRepository::validate_scopes(&req).expect("valid ok");
    assert_eq!(out, req);
}

#[test]
fn validate_scopes_rejects_invalid() {
    let req = vec!["user".to_owned(), "bogus".to_owned()];
    let err = OAuthRepository::validate_scopes(&req).expect_err("invalid scope");
    let msg = err.to_string();
    assert!(msg.contains("bogus"), "got: {msg}");
}

#[test]
fn scope_exists_known_and_unknown() {
    assert!(OAuthRepository::scope_exists("user"));
    assert!(OAuthRepository::scope_exists("admin"));
    assert!(OAuthRepository::scope_exists("anonymous"));
    assert!(!OAuthRepository::scope_exists("nope"));
}

#[test]
fn get_available_scopes_lists_all_three() {
    let scopes = OAuthRepository::get_available_scopes();
    assert_eq!(scopes.len(), 3);
    assert!(scopes.iter().all(|(_, desc)| desc.is_some()));
    assert!(scopes.iter().any(|(name, _)| name == "user"));
}

#[test]
fn parse_scopes_splits_whitespace_and_drops_empty() {
    let out = OAuthRepository::parse_scopes("  user   admin  ");
    assert_eq!(out, vec!["user".to_owned(), "admin".to_owned()]);
    assert!(OAuthRepository::parse_scopes("   ").is_empty());
}

#[test]
fn format_scopes_joins_with_space() {
    let out = OAuthRepository::format_scopes(&["user".to_owned(), "admin".to_owned()]);
    assert_eq!(out, "user admin");
    assert_eq!(OAuthRepository::format_scopes(&[]), "");
}

#[test]
fn get_default_roles_only_default_flagged() {
    let roles = OAuthRepository::get_default_roles();
    assert_eq!(roles, vec!["user".to_owned()]);
}

#[test]
fn generate_client_id_is_prefixed_and_unique() {
    let a = OAuthRepository::generate_client_id();
    let b = OAuthRepository::generate_client_id();
    assert!(a.starts_with("client_"), "got: {a}");
    assert_ne!(a, b);
}

#[test]
fn generate_client_secret_is_nonempty_and_unique() {
    let a = OAuthRepository::generate_client_secret();
    let b = OAuthRepository::generate_client_secret();
    assert!(!a.is_empty());
    assert_ne!(a, b);
}

#[test]
fn self_registration_may_not_claim_admin() {
    let err =
        OAuthRepository::validate_scopes_for_registration(&["user".to_owned(), "admin".to_owned()])
            .unwrap_err();
    assert!(err.to_string().contains("admin"), "{err}");
    assert!(!err.to_string().contains("user,"), "{err}");
}

#[test]
fn self_registration_accepts_user_and_anonymous() {
    let out = OAuthRepository::validate_scopes_for_registration(&[
        "user".to_owned(),
        "anonymous".to_owned(),
    ])
    .expect("both are self-registrable");
    assert_eq!(out, vec!["user".to_owned(), "anonymous".to_owned()]);
}

#[test]
fn self_registration_still_refuses_unknown_scopes() {
    assert!(OAuthRepository::validate_scopes_for_registration(&["superuser".to_owned()]).is_err());
}

#[test]
fn a_request_outside_the_client_scopes_is_refused() {
    let client = vec!["user".to_owned()];
    OAuthRepository::validate_scopes_for_client(&client, &["user".to_owned()])
        .expect("subset is fine");
    OAuthRepository::validate_scopes_for_client(&client, &[]).expect("empty is a subset");
    let err = OAuthRepository::validate_scopes_for_client(
        &client,
        &["user".to_owned(), "admin".to_owned()],
    )
    .unwrap_err();
    assert!(err.to_string().contains("admin"), "{err}");
}
