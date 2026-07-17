//! Database push / pull: serialise the user and context tables to
//! JSON and round-trip them between a local Postgres and a cloud Postgres
//! using compile-time-checked `sqlx` upserts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod upsert;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::prelude::FromRow;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_models::ContextKind;

use crate::error::SyncResult;
use crate::{SyncDirection, SyncOperationResult};

use upsert::{upsert_context, upsert_user};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseExport {
    pub users: Vec<UserExport>,
    pub contexts: Vec<ContextExport>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct UserExport {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct ContextExport {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub name: String,
    pub kind: ContextKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[derive(Debug)]
pub struct DatabaseSyncService {
    direction: SyncDirection,
    dry_run: bool,
    local_database_url: String,
    cloud_database_url: String,
}

impl DatabaseSyncService {
    pub fn new(
        direction: SyncDirection,
        dry_run: bool,
        local_database_url: &str,
        cloud_database_url: &str,
    ) -> Self {
        Self {
            direction,
            dry_run,
            local_database_url: local_database_url.to_owned(),
            cloud_database_url: cloud_database_url.to_owned(),
        }
    }

    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let export = export_from_database(&self.local_database_url).await?;
        let count = export.users.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_push",
                count,
                serde_json::json!({
                    "users": export.users.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        import_to_database(&self.cloud_database_url, &export).await?;
        Ok(SyncOperationResult::success("database_push", count))
    }

    async fn pull(&self) -> SyncResult<SyncOperationResult> {
        let export = export_from_database(&self.cloud_database_url).await?;
        let count = export.users.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_pull",
                count,
                serde_json::json!({
                    "users": export.users.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        import_to_database(&self.local_database_url, &export).await?;
        Ok(SyncOperationResult::success("database_pull", count))
    }
}

async fn export_from_database(database_url: &str) -> SyncResult<DatabaseExport> {
    let pool = PgPool::connect(database_url).await?;

    let users = sqlx::query_as!(
        UserExport,
        r#"SELECT id, name, email, full_name, display_name, status, email_verified,
                  roles, is_bot, is_scanner, avatar_url, created_at, updated_at
           FROM users"#
    )
    .fetch_all(&pool)
    .await?;

    let contexts = sqlx::query_as!(
        ContextExport,
        r#"SELECT context_id as "context_id!: ContextId",
                  user_id as "user_id!: UserId",
                  session_id as "session_id: SessionId",
                  name, kind as "kind: ContextKind", created_at, updated_at
           FROM user_contexts"#
    )
    .fetch_all(&pool)
    .await?;

    Ok(DatabaseExport {
        users,
        contexts,
        timestamp: Utc::now(),
    })
}

async fn import_to_database(
    database_url: &str,
    export: &DatabaseExport,
) -> SyncResult<ImportResult> {
    let pool = PgPool::connect(database_url).await?;
    let mut created = 0;
    let mut updated = 0;
    let mut completed = 0usize;
    let total = export.users.len() + export.contexts.len();

    for user in &export.users {
        match upsert_user(&pool, user).await {
            Ok((c, u)) => {
                created += c;
                updated += u;
                completed += 1;
            },
            Err(err) => {
                return Err(crate::error::SyncError::PartialImport {
                    completed,
                    total,
                    message: format!("user upsert failed: {err}"),
                });
            },
        }
    }

    for context in &export.contexts {
        match upsert_context(&pool, context).await {
            Ok((c, u)) => {
                created += c;
                updated += u;
                completed += 1;
            },
            Err(err) => {
                return Err(crate::error::SyncError::PartialImport {
                    completed,
                    total,
                    message: format!("context upsert failed: {err}"),
                });
            },
        }
    }

    Ok(ImportResult {
        created,
        updated,
        skipped: 0,
    })
}
