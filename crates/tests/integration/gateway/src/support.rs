use std::env;
use std::sync::{Arc, Once};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_config::SecretsBootstrap;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::UserId;
use uuid::Uuid;

pub fn minimal_request(system: Option<&str>, first_user_text: &str) -> CanonicalRequest {
    CanonicalRequest {
        model: "claude-test".to_string(),
        system: system.map(str::to_string),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text(first_user_text.to_string())],
        }],
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
    }
}

pub fn ensure_secrets_bootstrap() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY: single-threaded test init before any tokio runtime is up.
        unsafe {
            env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            if env::var("OAUTH_AT_REST_PEPPER").is_err() {
                env::set_var(
                    "OAUTH_AT_REST_PEPPER",
                    "test_oauth_at_rest_pepper_for_integration_tests_zzz",
                );
            }
            if env::var("MANIFEST_SIGNING_SECRET_SEED").is_err() {
                env::set_var(
                    "MANIFEST_SIGNING_SECRET_SEED",
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                );
            }
        }
        SecretsBootstrap::try_init().expect("SecretsBootstrap::try_init");
    });
}

pub async fn setup_db() -> DbPool {
    ensure_secrets_bootstrap();
    let url = env::var("DATABASE_URL").expect("DATABASE_URL required for gateway audit tests");
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test database");
    Arc::new(db)
}

pub async fn seed_user(db: &DbPool) -> UserId {
    let pool = db.pool_arc().expect("read pool");
    let unique = Uuid::new_v4().simple().to_string();
    let id = format!("gw-audit-user-{unique}");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&id)
        .bind(format!("{id}@test.invalid"))
        .execute(pool.as_ref())
        .await
        .expect("seed user");
    UserId::new(id)
}
