# Plan: WebAuthn Credential Linking for Existing Users

## Problem Statement

Admin users are auto-created in the database during CLI session initialization via `get_or_create_admin()`, but WebAuthn registration is designed only for NEW users. The current `finish_registration()` calls `create_user_with_webauthn_registration()` which fails with `"email_already_registered"` if the email exists.

**Result**: Admins cannot add WebAuthn credentials to their accounts.

## Solution: CLI-Generated Setup Token

Generate a one-time setup token via CLI that allows linking WebAuthn credentials to existing users in the browser.

---

## Part 1: Database Schema

**New file: `crates/domain/oauth/schema/webauthn_setup_tokens.sql`**

```sql
CREATE TABLE IF NOT EXISTS webauthn_setup_tokens (
    id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    purpose VARCHAR(50) NOT NULL CHECK(purpose IN ('credential_link', 'recovery')) DEFAULT 'credential_link',
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_token_hash ON webauthn_setup_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_user_id ON webauthn_setup_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_expires_at ON webauthn_setup_tokens(expires_at);
```

**Update: `crates/domain/oauth/src/extension.rs`**

Add to `schemas()`:
```rust
SchemaDefinition::inline(
    "webauthn_setup_tokens",
    include_str!("../schema/webauthn_setup_tokens.sql"),
)
.with_required_columns(vec!["id".into(), "user_id".into(), "token_hash".into()]),
```

---

## Part 2: Types

**New file: `crates/domain/oauth/src/types/setup_token.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SetupTokenPurpose {
    CredentialLink,
    Recovery,
}

impl SetupTokenPurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CredentialLink => "credential_link",
            Self::Recovery => "recovery",
        }
    }
}

impl std::fmt::Display for SetupTokenPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug)]
pub struct CreateSetupTokenParams {
    pub user_id: String,
    pub token_hash: String,
    pub purpose: SetupTokenPurpose,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SetupTokenRecord {
    pub id: String,
    pub user_id: String,
    pub purpose: SetupTokenPurpose,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum TokenValidationResult {
    Valid(SetupTokenRecord),
    Expired,
    AlreadyUsed,
    NotFound,
}
```

**Update: `crates/domain/oauth/src/types/mod.rs`**

```rust
pub mod setup_token;
pub use setup_token::*;
```

---

## Part 3: Repository Layer

**New file: `crates/domain/oauth/src/repository/setup_token.rs`**

```rust
use crate::types::{CreateSetupTokenParams, SetupTokenPurpose, SetupTokenRecord, TokenValidationResult};
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

#[instrument(skip(pool, params))]
pub async fn store_setup_token(pool: &PgPool, params: CreateSetupTokenParams) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO webauthn_setup_tokens (id, user_id, token_hash, purpose, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&id)
    .bind(&params.user_id)
    .bind(&params.token_hash)
    .bind(params.purpose.as_str())
    .bind(params.expires_at)
    .execute(pool)
    .await?;
    Ok(id)
}

#[instrument(skip(pool, token_hash))]
pub async fn validate_setup_token(
    pool: &PgPool,
    token_hash: &str,
) -> Result<TokenValidationResult> {
    let row = sqlx::query_as::<_, (String, String, String, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>, chrono::DateTime<Utc>)>(
        r#"
        SELECT id, user_id, purpose, expires_at, used_at, created_at
        FROM webauthn_setup_tokens
        WHERE token_hash = $1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(TokenValidationResult::NotFound),
        Some((id, user_id, purpose, expires_at, used_at, created_at)) => {
            if used_at.is_some() {
                return Ok(TokenValidationResult::AlreadyUsed);
            }
            if expires_at < Utc::now() {
                return Ok(TokenValidationResult::Expired);
            }
            let purpose = match purpose.as_str() {
                "credential_link" => SetupTokenPurpose::CredentialLink,
                "recovery" => SetupTokenPurpose::Recovery,
                _ => SetupTokenPurpose::CredentialLink,
            };
            Ok(TokenValidationResult::Valid(SetupTokenRecord {
                id, user_id, purpose, expires_at, created_at,
            }))
        }
    }
}

#[instrument(skip(pool))]
pub async fn consume_setup_token(pool: &PgPool, token_id: &str) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE webauthn_setup_tokens
        SET used_at = CURRENT_TIMESTAMP
        WHERE id = $1 AND used_at IS NULL
        "#,
    )
    .bind(token_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

#[instrument(skip(pool))]
pub async fn cleanup_expired_tokens(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM webauthn_setup_tokens
        WHERE (expires_at < CURRENT_TIMESTAMP - INTERVAL '24 hours')
           OR (used_at IS NOT NULL AND used_at < CURRENT_TIMESTAMP - INTERVAL '24 hours')
        "#,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[instrument(skip(pool))]
pub async fn revoke_user_tokens(pool: &PgPool, user_id: &str) -> Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE webauthn_setup_tokens
        SET used_at = CURRENT_TIMESTAMP
        WHERE user_id = $1 AND used_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
```

**Update: `crates/domain/oauth/src/repository/mod.rs`**

```rust
pub mod setup_token;
pub use setup_token::{
    store_setup_token,
    validate_setup_token,
    consume_setup_token,
    cleanup_expired_tokens,
    revoke_user_tokens,
};
```

---

## Part 4: Token Generation Utility

**New file: `crates/domain/oauth/src/services/webauthn/token.rs`**

```rust
use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

const TOKEN_PREFIX: &str = "sp_wst_";

pub fn generate_setup_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let raw_token = format!("{}{}", TOKEN_PREFIX, URL_SAFE_NO_PAD.encode(bytes));
    let hash = hash_token(&raw_token);
    (raw_token, hash)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    URL_SAFE_NO_PAD.encode(result)
}

pub fn validate_token_format(token: &str) -> Result<()> {
    if !token.starts_with(TOKEN_PREFIX) {
        anyhow::bail!("Invalid token format: missing prefix");
    }
    let encoded = token.strip_prefix(TOKEN_PREFIX).unwrap();
    if URL_SAFE_NO_PAD.decode(encoded).is_err() {
        anyhow::bail!("Invalid token format: invalid encoding");
    }
    Ok(())
}
```

**Update: `crates/domain/oauth/src/services/webauthn/mod.rs`**

```rust
pub mod token;
```

---

## Part 5: WebAuthn Service Extensions

**Update: `crates/domain/oauth/src/services/webauthn/service/mod.rs`**

Add field to `WebAuthnService`:
```rust
pub struct WebAuthnService {
    // ... existing fields ...
    link_registration_states: Arc<RwLock<HashMap<String, LinkRegistrationState>>>,
}

struct LinkRegistrationState {
    reg_state: PasskeyRegistration,
    user_id: String,
    token_id: String,
    timestamp: Instant,
}
```

**Update: `crates/domain/oauth/src/services/webauthn/service/registration.rs`**

Add methods:
```rust
use crate::repository::{validate_setup_token, consume_setup_token};
use crate::types::TokenValidationResult;
use crate::services::webauthn::token::hash_token;

impl WebAuthnService {
    pub async fn start_registration_with_token(
        &self,
        setup_token: &str,
    ) -> Result<(CreationChallengeResponse, String, UserInfo)> {
        let token_hash = hash_token(setup_token);
        let validation = validate_setup_token(&self.pool, &token_hash).await?;

        let token_record = match validation {
            TokenValidationResult::Valid(record) => record,
            TokenValidationResult::Expired => anyhow::bail!("Setup token has expired"),
            TokenValidationResult::AlreadyUsed => anyhow::bail!("Setup token has already been used"),
            TokenValidationResult::NotFound => anyhow::bail!("Invalid setup token"),
        };

        let user = self.user_service
            .get_user_by_id(&token_record.user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let existing_creds = self.get_user_credentials(&token_record.user_id).await?;
        let exclude_credentials: Vec<CredentialID> = existing_creds
            .iter()
            .map(|c| c.cred_id().clone())
            .collect();

        let user_unique_id = Uuid::parse_str(&token_record.user_id)
            .unwrap_or_else(|_| Uuid::new_v4());

        let (challenge, reg_state) = self.webauthn
            .start_passkey_registration(
                user_unique_id,
                &user.username,
                &user.display_name.unwrap_or_else(|| user.email.clone()),
                Some(exclude_credentials),
            )
            .map_err(|e| anyhow::anyhow!("WebAuthn error: {:?}", e))?;

        let challenge_id = Uuid::new_v4().to_string();
        let state = LinkRegistrationState {
            reg_state,
            user_id: token_record.user_id.clone(),
            token_id: token_record.id.clone(),
            timestamp: Instant::now(),
        };

        self.link_registration_states
            .write()
            .await
            .insert(challenge_id.clone(), state);

        Ok((challenge, challenge_id, user))
    }

    pub async fn finish_registration_with_token(
        &self,
        challenge_id: &str,
        setup_token: &str,
        credential: &RegisterPublicKeyCredential,
    ) -> Result<String> {
        let token_hash = hash_token(setup_token);
        let validation = validate_setup_token(&self.pool, &token_hash).await?;

        let token_record = match validation {
            TokenValidationResult::Valid(record) => record,
            _ => anyhow::bail!("Invalid or expired setup token"),
        };

        let state = self.link_registration_states
            .write()
            .await
            .remove(challenge_id)
            .ok_or_else(|| anyhow::anyhow!("Registration session not found or expired"))?;

        if state.token_id != token_record.id {
            anyhow::bail!("Token mismatch");
        }

        if state.timestamp.elapsed() > Duration::from_secs(CHALLENGE_EXPIRY_SECONDS) {
            anyhow::bail!("Registration session expired");
        }

        let passkey = self.webauthn
            .finish_passkey_registration(credential, &state.reg_state)
            .map_err(|e| anyhow::anyhow!("WebAuthn verification failed: {:?}", e))?;

        self.store_credential(&state.user_id, passkey, "Passkey").await?;
        consume_setup_token(&self.pool, &token_record.id).await?;

        tracing::info!(user_id = %state.user_id, "WebAuthn credential linked to existing user");
        Ok(state.user_id)
    }
}
```

---

## Part 6: API Endpoints

**New file: `crates/entry/api/src/routes/oauth/webauthn/link/mod.rs`**

```rust
mod start;
mod finish;

pub use start::start_link_credential;
pub use finish::finish_link_credential;
```

**New file: `crates/entry/api/src/routes/oauth/webauthn/link/start.rs`**

```rust
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StartLinkQuery {
    pub token: String,
}

pub async fn start_link_credential(
    State(state): State<AppState>,
    Query(query): Query<StartLinkQuery>,
) -> impl IntoResponse {
    let webauthn = match state.webauthn_service().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                Json(serde_json::json!({"error": "Service unavailable"})),
            );
        }
    };

    match webauthn.start_registration_with_token(&query.token).await {
        Ok((challenge, challenge_id, user)) => {
            let mut headers = HeaderMap::new();
            headers.insert("x-challenge-id", challenge_id.parse().unwrap());
            (StatusCode::OK, headers, Json(serde_json::json!({
                "challenge": challenge,
                "user": {
                    "id": user.id,
                    "email": user.email,
                    "display_name": user.display_name
                }
            })))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            HeaderMap::new(),
            Json(serde_json::json!({"error": e.to_string()})),
        )
    }
}
```

**New file: `crates/entry/api/src/routes/oauth/webauthn/link/finish.rs`**

```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

#[derive(Debug, Deserialize)]
pub struct FinishLinkRequest {
    pub challenge_id: String,
    pub token: String,
    pub credential: RegisterPublicKeyCredential,
}

pub async fn finish_link_credential(
    State(state): State<AppState>,
    Json(request): Json<FinishLinkRequest>,
) -> impl IntoResponse {
    let webauthn = match state.webauthn_service().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Service unavailable"})),
            );
        }
    };

    match webauthn
        .finish_registration_with_token(&request.challenge_id, &request.token, &request.credential)
        .await
    {
        Ok(user_id) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "user_id": user_id,
                "message": "Passkey registered successfully"
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    }
}
```

**Update: `crates/entry/api/src/routes/oauth/webauthn/mod.rs`**

```rust
pub mod link;
pub use link::{start_link_credential, finish_link_credential};
```

**Update: `crates/entry/api/src/routes/oauth/core.rs`**

Add to `public_router()`:
```rust
.route("/webauthn/link/start", get(webauthn::start_link_credential))
.route("/webauthn/link/finish", post(webauthn::finish_link_credential))
```

---

## Part 7: CLI Command

**New file: `crates/entry/cli/src/commands/admin/users/webauthn/mod.rs`**

```rust
mod generate_setup_token;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WebauthnCommands {
    GenerateSetupToken(generate_setup_token::Args),
}

pub async fn execute(cmd: WebauthnCommands, ctx: &crate::context::CliContext) -> anyhow::Result<()> {
    match cmd {
        WebauthnCommands::GenerateSetupToken(args) => {
            generate_setup_token::execute(args, ctx).await
        }
    }
}
```

**New file: `crates/entry/cli/src/commands/admin/users/webauthn/generate_setup_token.rs`**

```rust
use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args as ClapArgs;
use colored::Colorize;

use systemprompt_oauth::repository::store_setup_token;
use systemprompt_oauth::services::webauthn::token::generate_setup_token;
use systemprompt_oauth::types::{CreateSetupTokenParams, SetupTokenPurpose};

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub email: String,

    #[arg(long, default_value = "15")]
    pub expires_minutes: u32,
}

pub async fn execute(args: Args, ctx: &crate::context::CliContext) -> Result<()> {
    let pool = ctx.db_pool();

    let user = sqlx::query_as::<_, (String, String)>(
        "SELECT id, email FROM users WHERE email = $1",
    )
    .bind(&args.email)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("User not found: {}", args.email))?;

    let (user_id, email) = user;
    let (raw_token, token_hash) = generate_setup_token();
    let expires_at = Utc::now() + Duration::minutes(args.expires_minutes as i64);

    let params = CreateSetupTokenParams {
        user_id: user_id.clone(),
        token_hash,
        purpose: SetupTokenPurpose::CredentialLink,
        expires_at,
    };

    store_setup_token(pool, params).await?;

    let external_url = std::env::var("API_EXTERNAL_URL")
        .unwrap_or_else(|_| "https://your-domain.com".to_string());
    let link_url = format!("{}/auth/link-passkey?token={}", external_url, raw_token);

    println!();
    println!("{}", "Setup Token Generated".bold().green());
    println!("{}", "─".repeat(50));
    println!("User:     {}", email.cyan());
    println!("Expires:  {} minutes", args.expires_minutes);
    println!();
    println!("{}", "Token:".bold());
    println!("  {}", raw_token.yellow());
    println!();
    println!("{}", "Registration URL:".bold());
    println!("  {}", link_url.blue().underline());
    println!();

    Ok(())
}
```

**Update: `crates/entry/cli/src/commands/admin/users/mod.rs`**

```rust
pub mod webauthn;

#[derive(Debug, Subcommand)]
pub enum UsersCommands {
    // ... existing commands ...
    #[command(subcommand)]
    Webauthn(webauthn::WebauthnCommands),
}

pub async fn execute(cmd: UsersCommands, ctx: &CliContext) -> Result<()> {
    match cmd {
        // ... existing handlers ...
        UsersCommands::Webauthn(subcmd) => webauthn::execute(subcmd, ctx).await,
    }
}
```

---

## Files Summary

### New Files

| File | Description |
|------|-------------|
| `crates/domain/oauth/schema/webauthn_setup_tokens.sql` | Database schema |
| `crates/domain/oauth/src/types/setup_token.rs` | Token types |
| `crates/domain/oauth/src/repository/setup_token.rs` | Repository layer |
| `crates/domain/oauth/src/services/webauthn/token.rs` | Token utilities |
| `crates/entry/api/src/routes/oauth/webauthn/link/mod.rs` | Route module |
| `crates/entry/api/src/routes/oauth/webauthn/link/start.rs` | Start endpoint |
| `crates/entry/api/src/routes/oauth/webauthn/link/finish.rs` | Finish endpoint |
| `crates/entry/cli/src/commands/admin/users/webauthn/mod.rs` | CLI module |
| `crates/entry/cli/src/commands/admin/users/webauthn/generate_setup_token.rs` | CLI command |

### Files to Update

| File | Change |
|------|--------|
| `crates/domain/oauth/src/extension.rs` | Register schema |
| `crates/domain/oauth/src/types/mod.rs` | Export setup_token |
| `crates/domain/oauth/src/repository/mod.rs` | Export setup_token |
| `crates/domain/oauth/src/services/webauthn/mod.rs` | Export token module |
| `crates/domain/oauth/src/services/webauthn/service/mod.rs` | Add state field |
| `crates/domain/oauth/src/services/webauthn/service/registration.rs` | Add token methods |
| `crates/entry/api/src/routes/oauth/webauthn/mod.rs` | Export link module |
| `crates/entry/api/src/routes/oauth/core.rs` | Add link routes |
| `crates/entry/cli/src/commands/admin/users/mod.rs` | Add webauthn subcommand |

---

## Verification Steps

1. `just migrate` - Run migrations
2. `just build` - Build the project
3. `systemprompt admin users webauthn generate-setup-token --email admin@example.com`
4. Open the URL in browser
5. Complete WebAuthn ceremony
6. Verify: `SELECT * FROM webauthn_credentials WHERE user_id = '...'`
7. Test auth: `/oauth/webauthn/authenticate/start?email=admin@example.com`
