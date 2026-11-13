use systemprompt_extension::AuthzPolicy;
use systemprompt_models::auth::UserType;

const ALL: [UserType; 6] = [
    UserType::Anon,
    UserType::User,
    UserType::Admin,
    UserType::A2a,
    UserType::Mcp,
    UserType::Service,
];

fn admitted(policy: AuthzPolicy) -> Vec<UserType> {
    ALL.into_iter().filter(|t| policy.permits(*t)).collect()
}

#[test]
fn public_admits_every_caller_type() {
    assert_eq!(admitted(AuthzPolicy::public()), ALL.to_vec());
}

#[test]
fn authenticated_admits_everyone_but_anon() {
    assert_eq!(admitted(AuthzPolicy::authenticated()), ALL[1..].to_vec());
}

#[test]
fn user_admits_user_and_admin() {
    assert_eq!(
        admitted(AuthzPolicy::user()),
        vec![UserType::User, UserType::Admin]
    );
}

#[test]
fn admin_admits_only_admin() {
    assert_eq!(admitted(AuthzPolicy::admin()), vec![UserType::Admin]);
}

#[test]
fn restricted_to_admits_exactly_its_list() {
    let policy = AuthzPolicy::restricted_to(&[UserType::Service]);
    assert_eq!(admitted(policy), vec![UserType::Service]);
    assert_eq!(policy.allowed(), &[UserType::Service]);
}

#[test]
fn deferred_to_handler_admits_what_public_admits() {
    assert_eq!(
        admitted(AuthzPolicy::deferred_to_handler()),
        admitted(AuthzPolicy::public())
    );
}
