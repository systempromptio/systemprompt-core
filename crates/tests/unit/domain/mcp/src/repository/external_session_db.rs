//! External provider session ownership, expiry, and persistence across
//! repository instances.

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_mcp::repository::{ExternalSessionBinding, McpProxyIdentityRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn external_session_rejects_other_users_credentials_and_servers()
-> Result<(), Box<dyn std::error::Error>> {
    let db = fixture_db_pool(&fixture_database_url()?).await?;
    let first = McpProxyIdentityRepository::new(&db)?;
    let second = McpProxyIdentityRepository::new(&db)?;
    let session_id = SessionId::generate();
    let user_id = UserId::new("external-owner");
    let other_user = UserId::new("external-intruder");
    let binding = ExternalSessionBinding {
        server: "external-test",
        session_id: &session_id,
        user_id: &user_id,
        credential_hash: b"original",
    };
    assert!(!first.accepts_external(&binding).await?);
    assert!(first.remember_external(&binding).await?);
    assert!(
        second.accepts_external(&binding).await?,
        "another replica recognizes the binding"
    );
    let wrong_user = ExternalSessionBinding {
        user_id: &other_user,
        ..binding
    };
    let wrong_credential = ExternalSessionBinding {
        credential_hash: b"rotated",
        ..binding
    };
    let wrong_server = ExternalSessionBinding {
        server: "another-server",
        ..binding
    };
    for invalid in [&wrong_user, &wrong_credential, &wrong_server] {
        assert!(!second.accepts_external(invalid).await?);
        second.forget_external(invalid).await?;
        assert!(
            first.accepts_external(&binding).await?,
            "an invalid caller cannot delete the binding"
        );
    }
    assert!(
        !second.remember_external(&wrong_user).await?,
        "a provider collision cannot transfer an active session"
    );
    assert!(!second.remember_external(&wrong_credential).await?);
    assert!(second.accepts_external(&binding).await?);
    second.forget_external(&binding).await?;
    assert!(!first.accepts_external(&binding).await?);
    Ok(())
}

#[tokio::test]
async fn external_session_expiry_is_enforced_and_can_be_reinitialized()
-> Result<(), Box<dyn std::error::Error>> {
    let db = fixture_db_pool(&fixture_database_url()?).await?;
    let repo = McpProxyIdentityRepository::new(&db)?;
    let session_id = SessionId::generate();
    let user_id = UserId::new("external-expiry-owner");
    let binding = ExternalSessionBinding {
        server: "expiry-test",
        session_id: &session_id,
        user_id: &user_id,
        credential_hash: b"original",
    };
    assert!(repo.remember_external(&binding).await?);
    sqlx::query("UPDATE mcp_external_sessions SET expires_at = NOW() - INTERVAL '1 second' WHERE session_id = $1")
        .bind(session_id.as_str()).execute(db.write_pool_arc()?.as_ref()).await?;
    assert!(!repo.accepts_external(&binding).await?);
    let refreshed = ExternalSessionBinding {
        credential_hash: b"refreshed",
        ..binding
    };
    assert!(repo.remember_external(&refreshed).await?);
    assert!(!repo.accepts_external(&binding).await?);
    assert!(repo.accepts_external(&refreshed).await?);
    repo.forget_external(&refreshed).await?;
    Ok(())
}
