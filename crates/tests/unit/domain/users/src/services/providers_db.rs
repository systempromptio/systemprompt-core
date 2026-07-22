//! DB-backed tests for the `UserProvider` and `RoleProvider` trait
//! implementations on `UserService`, including closed-pool error mapping.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_traits::auth::AuthProviderError;
use systemprompt_users::{RoleProvider, UserProvider, UserService};
use uuid::Uuid;

async fn setup() -> Option<UserService> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(UserService::new(&pool).expect("service"))
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@prov.invalid"),
    )
}

#[tokio::test]
async fn user_provider_creates_and_finds_auth_users() {
    let Some(service) = setup().await else {
        return;
    };
    let (name, email) = unique("provider");

    let created = UserProvider::create_user(&service, &name, &email, Some("Full Prov"))
        .await
        .expect("create_user");
    assert_eq!(created.name, name);
    assert!(created.is_active);
    assert_eq!(created.roles, vec!["user".to_owned()]);

    let by_id = UserProvider::find_by_id(&service, &created.id)
        .await
        .expect("find_by_id")
        .expect("row");
    assert_eq!(by_id.email, email);

    let by_email = UserProvider::find_by_email(&service, &email)
        .await
        .expect("find_by_email")
        .expect("row");
    assert_eq!(by_email.id, created.id);

    let by_name = UserProvider::find_by_name(&service, &name)
        .await
        .expect("find_by_name")
        .expect("row");
    assert_eq!(by_name.id, created.id);

    UserProvider::assign_roles(
        &service,
        &created.id,
        &["user".to_owned(), "admin".to_owned()],
    )
    .await
    .expect("assign_roles");
    let roles = RoleProvider::get_roles(&service, &created.id)
        .await
        .expect("get_roles");
    assert!(roles.contains(&"admin".to_owned()));

    service.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn user_provider_creates_anonymous_and_federated_identities() {
    let Some(service) = setup().await else {
        return;
    };
    let fingerprint = format!("prov-anon-{}", Uuid::new_v4().simple());
    let anon = UserProvider::create_anonymous(&service, &fingerprint)
        .await
        .expect("create_anonymous");
    assert!(anon.roles.contains(&"anonymous".to_owned()));

    let issuer = format!("https://idp-{}.example.com", Uuid::new_v4().simple());
    let claims = FederatedIdentityClaims {
        email: Some("fed-prov@example.com".to_owned()),
        email_verified: true,
        name: Some("Fed Prov".to_owned()),
        preferred_username: Some("fedprov".to_owned()),
        roles: vec!["operator".to_owned()],
    };
    let fed_id = UserProvider::find_or_create_federated(&service, &issuer, "sub-prov", &claims)
        .await
        .expect("federated");
    let fed_user = service
        .find_by_id(&fed_id)
        .await
        .expect("find fed")
        .expect("row");
    assert!(fed_user.name.starts_with("fedprov"));
    assert_eq!(fed_user.roles, vec!["operator".to_owned()]);

    service.delete(&anon.id).await.expect("cleanup anon");
    service.delete(&fed_id).await.expect("cleanup fed");
}

#[tokio::test]
async fn role_provider_assign_and_revoke_are_idempotent() {
    let Some(service) = setup().await else {
        return;
    };
    let (name, email) = unique("roles");
    let created = UserProvider::create_user(&service, &name, &email, None)
        .await
        .expect("create");

    RoleProvider::assign_role(&service, &created.id, "auditor")
        .await
        .expect("assign");
    RoleProvider::assign_role(&service, &created.id, "auditor")
        .await
        .expect("re-assign");
    let roles = RoleProvider::get_roles(&service, &created.id)
        .await
        .expect("roles");
    assert_eq!(roles.iter().filter(|r| *r == "auditor").count(), 1);

    RoleProvider::revoke_role(&service, &created.id, "auditor")
        .await
        .expect("revoke");
    let after = RoleProvider::get_roles(&service, &created.id)
        .await
        .expect("roles after");
    assert!(!after.contains(&"auditor".to_owned()));

    service.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn role_provider_lists_by_role_and_ignores_unknown_roles() {
    let Some(service) = setup().await else {
        return;
    };
    let (name, email) = unique("byrole");
    let created = UserProvider::create_user(&service, &name, &email, None)
        .await
        .expect("create");
    RoleProvider::assign_role(&service, &created.id, "admin")
        .await
        .expect("grant admin");

    let admins = RoleProvider::list_users_by_role(&service, "admin")
        .await
        .expect("admins");
    assert!(admins.iter().any(|u| u.id == created.id));

    let unknown = RoleProvider::list_users_by_role(&service, "not-a-real-role")
        .await
        .expect("unknown role");
    assert!(unknown.is_empty());

    service.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn missing_user_maps_to_user_not_found() {
    let Some(service) = setup().await else {
        return;
    };
    let ghost = UserId::new(Uuid::new_v4().to_string());

    assert!(matches!(
        RoleProvider::get_roles(&service, &ghost).await,
        Err(AuthProviderError::UserNotFound)
    ));
    assert!(matches!(
        RoleProvider::assign_role(&service, &ghost, "admin").await,
        Err(AuthProviderError::UserNotFound)
    ));
    assert!(matches!(
        RoleProvider::revoke_role(&service, &ghost, "admin").await,
        Err(AuthProviderError::UserNotFound)
    ));
}

#[tokio::test]
async fn closed_pool_maps_to_internal_errors() {
    ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let service = UserService::new(&pool).expect("service");
    let ghost = UserId::new(Uuid::new_v4().to_string());

    assert!(matches!(
        UserProvider::find_by_id(&service, &ghost).await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        UserProvider::find_by_email(&service, "x@closed.invalid").await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        UserProvider::find_by_name(&service, "closed").await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        UserProvider::create_user(&service, "closed", "c@closed.invalid", None).await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        UserProvider::create_anonymous(&service, "closed-fp").await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        UserProvider::assign_roles(&service, &ghost, &["user".to_owned()]).await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        RoleProvider::get_roles(&service, &ghost).await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        RoleProvider::assign_role(&service, &ghost, "admin").await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        RoleProvider::revoke_role(&service, &ghost, "admin").await,
        Err(AuthProviderError::Internal(_))
    ));
    assert!(matches!(
        RoleProvider::list_users_by_role(&service, "admin").await,
        Err(AuthProviderError::Internal(_))
    ));
}
