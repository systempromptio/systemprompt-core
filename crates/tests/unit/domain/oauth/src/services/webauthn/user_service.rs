use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::services::UserCreationService;
use systemprompt_traits::{AuthResult, AuthUser, UserProvider};

struct MockUserProvider {
    users: Mutex<Vec<AuthUser>>,
    created_users: Mutex<Vec<AuthUser>>,
    assigned_roles: Mutex<Vec<(String, Vec<String>)>>,
}

impl MockUserProvider {
    fn new() -> Self {
        Self {
            users: Mutex::new(Vec::new()),
            created_users: Mutex::new(Vec::new()),
            assigned_roles: Mutex::new(Vec::new()),
        }
    }

    fn with_existing_user(self, user: AuthUser) -> Self {
        self.users.lock().expect("lock poisoned").push(user);
        self
    }
}

#[async_trait]
impl UserProvider for MockUserProvider {
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>> {
        let users = self.users.lock().expect("lock poisoned");
        Ok(users.iter().find(|u| u.id.as_str() == id.as_str()).cloned())
    }

    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>> {
        let users = self.users.lock().expect("lock poisoned");
        Ok(users.iter().find(|u| u.email == email).cloned())
    }

    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>> {
        let users = self.users.lock().expect("lock poisoned");
        Ok(users.iter().find(|u| u.name == name).cloned())
    }

    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        let user = AuthUser {
            id: UserId::new(uuid::Uuid::new_v4().to_string()),
            name: name.to_string(),
            email: email.to_string(),
            roles: Vec::new(),
            is_active: true,
        };
        self.created_users
            .lock()
            .expect("lock poisoned")
            .push(user.clone());
        if full_name.is_some() {
            let mut users = self.users.lock().expect("lock poisoned");
            users.push(AuthUser {
                id: user.id.clone(),
                name: full_name.unwrap_or(name).to_string(),
                email: email.to_string(),
                roles: Vec::new(),
                is_active: true,
            });
        }
        Ok(user)
    }

    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new(uuid::Uuid::new_v4().to_string()),
            name: format!("anon-{fingerprint}"),
            email: String::new(),
            roles: Vec::new(),
            is_active: true,
        })
    }

    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()> {
        self.assigned_roles
            .lock()
            .expect("lock poisoned")
            .push((user_id.as_str().to_string(), roles.to_vec()));
        Ok(())
    }
}

fn make_test_user(id: &str, name: &str, email: &str) -> AuthUser {
    AuthUser {
        id: UserId::new(id),
        name: name.to_string(),
        email: email.to_string(),
        roles: vec!["user".to_string()],
        is_active: true,
    }
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_user_creation_service_new() {
    let provider = Arc::new(MockUserProvider::new());
    let _service = UserCreationService::new(provider);
}

#[test]
fn test_user_creation_service_debug() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider);
    let debug_output = format!("{service:?}");
    assert!(
        debug_output.contains("UserCreationService"),
        "debug output should contain struct name"
    );
}

// ============================================================================
// find_or_create_user_with_webauthn_registration Tests
// ============================================================================

#[tokio::test]
async fn test_find_or_create_existing_user_by_email() {
    let existing = make_test_user("existing-id-123", "alice", "alice@example.com");
    let provider = Arc::new(MockUserProvider::new().with_existing_user(existing));
    let service = UserCreationService::new(provider.clone());

    let result = service
        .find_or_create_user_with_webauthn_registration("alice", "alice@example.com", None, None)
        .await
        .expect("should succeed for existing user");

    assert_eq!(result, "existing-id-123");
    assert!(
        provider.created_users.lock().expect("lock").is_empty(),
        "should not create a new user when one exists"
    );
}

#[tokio::test]
async fn test_find_or_create_new_user() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let result = service
        .find_or_create_user_with_webauthn_registration("newuser", "new@example.com", None, None)
        .await
        .expect("should succeed for new user");

    assert!(!result.is_empty(), "should return a non-empty user ID");
    assert_eq!(
        provider.created_users.lock().expect("lock").len(),
        1,
        "should have created exactly one user"
    );
}

#[tokio::test]
async fn test_find_or_create_assigns_default_roles() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let user_id = service
        .find_or_create_user_with_webauthn_registration(
            "defaultrole",
            "default@example.com",
            None,
            None,
        )
        .await
        .expect("should succeed");

    let roles = provider.assigned_roles.lock().expect("lock");
    assert_eq!(roles.len(), 1, "should have assigned roles once");
    assert_eq!(roles[0].0, user_id, "roles assigned to correct user");
    assert_eq!(
        roles[0].1,
        vec!["user".to_string()],
        "default role should be 'user'"
    );
}

#[tokio::test]
async fn test_find_or_create_assigns_custom_roles() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let custom_roles = vec!["admin".to_string(), "editor".to_string()];
    let user_id = service
        .find_or_create_user_with_webauthn_registration(
            "customrole",
            "custom@example.com",
            None,
            Some(custom_roles.clone()),
        )
        .await
        .expect("should succeed");

    let roles = provider.assigned_roles.lock().expect("lock");
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].0, user_id);
    assert_eq!(roles[0].1, custom_roles, "should use provided custom roles");
}

#[tokio::test]
async fn test_find_or_create_passes_full_name() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    service
        .find_or_create_user_with_webauthn_registration(
            "nameduser",
            "named@example.com",
            Some("Alice Wonderland"),
            None,
        )
        .await
        .expect("should succeed");

    let created = provider.created_users.lock().expect("lock");
    assert_eq!(created.len(), 1, "should have created one user");
    let stored = provider.users.lock().expect("lock");
    assert!(
        stored.iter().any(|u| u.name == "Alice Wonderland"),
        "full_name should be forwarded to create_user"
    );
}

#[tokio::test]
async fn test_find_or_create_returns_created_user_id() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let returned_id = service
        .find_or_create_user_with_webauthn_registration(
            "idcheck",
            "idcheck@example.com",
            None,
            None,
        )
        .await
        .expect("should succeed");

    let created = provider.created_users.lock().expect("lock");
    assert_eq!(created.len(), 1);
    assert_eq!(
        returned_id,
        created[0].id.as_str(),
        "returned ID should match the created user's ID"
    );
}

// ============================================================================
// create_user_with_webauthn_registration Tests
// ============================================================================

#[tokio::test]
async fn test_create_user_email_already_registered() {
    let existing = make_test_user("existing-1", "alice", "alice@example.com");
    let provider = Arc::new(MockUserProvider::new().with_existing_user(existing));
    let service = UserCreationService::new(provider);

    let result = service
        .create_user_with_webauthn_registration("differentname", "alice@example.com", None)
        .await;

    let err = result.expect_err("should fail for existing email");
    assert!(
        err.to_string().contains("email_already_registered"),
        "error should contain 'email_already_registered', got: {err}"
    );
}

#[tokio::test]
async fn test_create_user_username_taken() {
    let existing = make_test_user("existing-2", "takenname", "other@example.com");
    let provider = Arc::new(MockUserProvider::new().with_existing_user(existing));
    let service = UserCreationService::new(provider);

    let result = service
        .create_user_with_webauthn_registration("takenname", "fresh@example.com", None)
        .await;

    let err = result.expect_err("should fail for taken username");
    assert!(
        err.to_string().contains("username_already_taken"),
        "error should contain 'username_already_taken', got: {err}"
    );
}

#[tokio::test]
async fn test_create_user_success() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let result = service
        .create_user_with_webauthn_registration("brandnew", "brandnew@example.com", None)
        .await
        .expect("should succeed for new user");

    assert!(!result.is_empty(), "should return a non-empty user ID");
    let created = provider.created_users.lock().expect("lock");
    assert_eq!(created.len(), 1, "should have created exactly one user");
    assert_eq!(created[0].name, "brandnew");
    assert_eq!(created[0].email, "brandnew@example.com");
}

#[tokio::test]
async fn test_create_user_email_check_before_username() {
    let existing = make_test_user("dual-1", "sharedname", "shared@example.com");
    let provider = Arc::new(MockUserProvider::new().with_existing_user(existing));
    let service = UserCreationService::new(provider);

    let result = service
        .create_user_with_webauthn_registration("sharedname", "shared@example.com", None)
        .await;

    let err = result.expect_err("should fail");
    assert!(
        err.to_string().contains("email_already_registered"),
        "email check should come before username check, got: {err}"
    );
}

#[tokio::test]
async fn test_create_user_passes_full_name() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    service
        .create_user_with_webauthn_registration(
            "fullnamed",
            "fullnamed@example.com",
            Some("Bob Builder"),
        )
        .await
        .expect("should succeed");

    let stored = provider.users.lock().expect("lock");
    assert!(
        stored.iter().any(|u| u.name == "Bob Builder"),
        "full_name should be forwarded through to create_user"
    );
}

#[tokio::test]
async fn test_create_user_none_full_name() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let result = service
        .create_user_with_webauthn_registration("nofullname", "nofullname@example.com", None)
        .await;

    result.expect("should succeed with None full_name");
    let created = provider.created_users.lock().expect("lock");
    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].name, "nofullname",
        "username should be used when full_name is None"
    );
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_find_or_create_empty_username() {
    let provider = Arc::new(MockUserProvider::new());
    let service = UserCreationService::new(provider.clone());

    let result = service
        .find_or_create_user_with_webauthn_registration("", "empty@example.com", None, None)
        .await;

    assert!(
        result.is_ok(),
        "empty username should be accepted by find_or_create (validation is upstream)"
    );
    let created = provider.created_users.lock().expect("lock");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].name, "");
}
