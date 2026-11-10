use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::{
    ActiveSession, AnalyticsResult, AnalyticsSession, AuthResult, AuthUser, CreateSessionInput,
    FederatedIdentityClaims, FederatedLink, FederatedRefusal, FederatedResolution, LinkPolicy,
    SessionProvider, UserProvider,
};

#[derive(Debug, Default)]
pub struct MemoryUsers {
    pub users: Mutex<HashMap<UserId, AuthUser>>,
    pub federated: Mutex<HashMap<(String, String), (UserId, bool)>>,
    pub assigned: Mutex<HashMap<UserId, Vec<String>>>,
}

pub fn auth_user(id: &str, email: &str) -> AuthUser {
    AuthUser {
        id: UserId::new(id),
        name: id.to_owned(),
        email: email.to_owned(),
        roles: vec!["user".to_owned()],
        is_active: true,
        is_service_account: false,
    }
}

impl MemoryUsers {
    pub fn seed(&self, user: AuthUser) {
        self.users
            .lock()
            .expect("users lock")
            .insert(user.id.clone(), user);
    }

    pub fn seed_link(&self, issuer: &str, sub: &str, user_id: &UserId, provisioned: bool) {
        self.federated.lock().expect("federated lock").insert(
            (issuer.to_owned(), sub.to_owned()),
            (user_id.clone(), provisioned),
        );
    }

    pub fn linked(&self, issuer: &str, sub: &str) -> UserId {
        self.federated.lock().expect("federated lock")[&(issuer.to_owned(), sub.to_owned())]
            .0
            .clone()
    }

    pub fn roles_of(&self, user_id: &UserId) -> Option<Vec<String>> {
        self.assigned
            .lock()
            .expect("assigned lock")
            .get(user_id)
            .cloned()
    }

    pub fn link_count(&self) -> usize {
        self.federated.lock().expect("federated lock").len()
    }
}

#[async_trait]
impl UserProvider for MemoryUsers {
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(self.users.lock().expect("users lock").get(id).cloned())
    }

    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(self
            .users
            .lock()
            .expect("users lock")
            .values()
            .find(|user| user.email == email)
            .cloned())
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
        Ok(auth_user(name, email))
    }

    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(auth_user(fingerprint, "anon@example.test"))
    }

    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()> {
        self.assigned
            .lock()
            .expect("assigned lock")
            .insert(user_id.clone(), roles.to_vec());
        Ok(())
    }

    async fn resolve_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
        policy: &LinkPolicy,
    ) -> AuthResult<FederatedResolution> {
        let existing = self
            .federated
            .lock()
            .expect("federated lock")
            .get(&(issuer.to_owned(), external_sub.to_owned()))
            .cloned();
        if let Some((user_id, provisioned)) = existing {
            return Ok(FederatedResolution {
                user_id,
                link: FederatedLink::Existing { provisioned },
            });
        }
        if !policy.auto_provision {
            return Err(FederatedRefusal::NotProvisioned.into());
        }
        let user_id = UserId::new(format!("user_{}", uuid::Uuid::new_v4()));
        self.seed(auth_user(
            user_id.as_str(),
            claims.email.as_deref().unwrap_or_default(),
        ));
        self.seed_link(issuer, external_sub, &user_id, true);
        Ok(FederatedResolution {
            user_id,
            link: FederatedLink::Provisioned,
        })
    }

    async fn promote_anonymous(&self, _source: &UserId, _target: &UserId) -> AuthResult<u64> {
        Ok(0)
    }
}

#[derive(Debug, Default)]
pub struct CountingSessions {
    pub created: Mutex<Vec<(SessionId, Option<UserId>)>>,
}

#[async_trait]
impl SessionProvider for CountingSessions {
    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        self.created
            .lock()
            .expect("sessions lock")
            .push((input.session_id.clone(), input.user_id.cloned()));
        Ok(())
    }

    async fn find_recent_session_by_fingerprint(
        &self,
        _fingerprint: &str,
        _max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Ok(None)
    }

    async fn find_session_by_id(
        &self,
        _session_id: &SessionId,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Ok(None)
    }

    async fn find_active_session_by_id(
        &self,
        _session_id: &SessionId,
    ) -> AnalyticsResult<Option<ActiveSession>> {
        Ok(None)
    }

    async fn revoke_session(&self, _session_id: &SessionId) -> AnalyticsResult<()> {
        Ok(())
    }

    async fn revoke_all_sessions_for_user(&self, _user_id: &UserId) -> AnalyticsResult<u64> {
        Ok(0)
    }

    async fn migrate_user_sessions(
        &self,
        _from_user_id: &UserId,
        _to_user_id: &UserId,
    ) -> AnalyticsResult<u64> {
        Ok(0)
    }

    async fn mark_session_converted(&self, _session_id: &SessionId) -> AnalyticsResult<()> {
        Ok(())
    }
}
