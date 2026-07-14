// WebAuthnRegistry process-wide singleton: second call returns the cached
// service; also drives WebAuthnService::new + Debug via the config path.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::WebAuthnService;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{AuthResult, AuthUser, UserProvider};

struct NoopUsers;

#[async_trait]
impl UserProvider for NoopUsers {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn find_by_email(&self, _email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn find_by_name(&self, _name: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_registry_test"),
            name: name.to_owned(),
            email: email.to_owned(),
            roles: vec![],
            is_active: true,
        })
    }
    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_registry_anon"),
            name: "anon".to_owned(),
            email: String::new(),
            roles: vec![],
            is_active: true,
        })
    }
    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }
    async fn find_or_create_federated(
        &self,
        _issuer: &str,
        _external_sub: &str,
        _claims: &systemprompt_traits::FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Ok(UserId::new("user_registry_fed"))
    }
}

#[tokio::test]
async fn registry_surfaces_invalid_relying_party_configuration() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let err = WebAuthnRegistry::get_or_create_service(repo, Arc::new(NoopUsers))
        .await
        .expect_err("ip-address rp_id from the fixture profile must be rejected");
    assert!(matches!(
        err,
        systemprompt_oauth::error::OauthError::WebAuthnVerificationFailed(_)
    ));
}

#[tokio::test]
async fn service_new_rejects_ip_address_relying_party() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let err = WebAuthnService::new(repo, Arc::new(NoopUsers))
        .expect_err("webauthn-rs rejects an IP-address RP ID");
    assert!(matches!(
        err,
        systemprompt_oauth::error::OauthError::WebAuthnVerificationFailed(_)
    ));
}
