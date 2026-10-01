// WebAuthnService construction through the config path: an IP-address relying
// party is rejected when the composition root builds the service.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::WebAuthnService;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
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

    async fn promote_anonymous(&self, _source: &UserId, _target: &UserId) -> AuthResult<u64> {
        Ok(0)
    }
}

#[tokio::test]
async fn service_new_rejects_ip_address_relying_party() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let err = WebAuthnService::new(repo, Arc::new(NoopUsers))
        .expect_err("webauthn-rs rejects an IP-address RP ID");
    assert!(matches!(
        err,
        systemprompt_oauth::error::OauthError::WebAuthnCeremony(_)
    ));
}
