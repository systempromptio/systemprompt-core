use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_secrets_bootstrap, fixture_database_url, fixture_db_pool,
};
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

pub async fn setup_db() -> DbPool {
    ensure_test_secrets_bootstrap();
    let url = fixture_database_url().expect("DATABASE_URL required for gateway audit tests");
    fixture_db_pool(&url).await.expect("connect to test database")
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
