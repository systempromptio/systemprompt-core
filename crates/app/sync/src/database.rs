use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;
use crate::{SyncConfig, SyncDirection, SyncOperationResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseExport {
    pub agents: Vec<AgentExport>,
    pub skills: Vec<SkillExport>,
    pub contexts: Vec<ContextExport>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentExport {
    pub id: Uuid,
    pub name: String,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SkillExport {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContextExport {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[derive(Debug)]
pub struct DatabaseSyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
    database_url: String,
}

impl DatabaseSyncService {
    pub fn new(config: SyncConfig, api_client: SyncApiClient, database_url: &str) -> Self {
        Self {
            config,
            api_client,
            database_url: database_url.to_string(),
        }
    }

    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.config.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let export = self.export_local().await?;
        let count = export.agents.len() + export.skills.len() + export.contexts.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_push",
                count,
                serde_json::json!({
                    "agents": export.agents.len(),
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        self.api_client
            .import_database(&self.config.tenant_id, &export)
            .await?;

        Ok(SyncOperationResult::success("database_push", count))
    }

    async fn pull(&self) -> SyncResult<SyncOperationResult> {
        let export = self
            .api_client
            .export_database(&self.config.tenant_id)
            .await?;

        let count = export.agents.len() + export.skills.len() + export.contexts.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_pull",
                count,
                serde_json::json!({
                    "agents": export.agents.len(),
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        self.import_local(&export).await?;
        Ok(SyncOperationResult::success("database_pull", count))
    }

    async fn export_local(&self) -> SyncResult<DatabaseExport> {
        let pool = PgPool::connect(&self.database_url).await?;

        let agents: Vec<AgentExport> =
            sqlx::query_as("SELECT id, name, system_prompt, created_at, updated_at FROM agents")
                .fetch_all(&pool)
                .await?;

        let skills: Vec<SkillExport> = sqlx::query_as(
            "SELECT id, agent_id, name, description, created_at, updated_at FROM agent_skills",
        )
        .fetch_all(&pool)
        .await?;

        let contexts: Vec<ContextExport> =
            sqlx::query_as("SELECT id, name, description, created_at, updated_at FROM contexts")
                .fetch_all(&pool)
                .await?;

        Ok(DatabaseExport {
            agents,
            skills,
            contexts,
            timestamp: Utc::now(),
        })
    }

    async fn import_local(&self, export: &DatabaseExport) -> SyncResult<ImportResult> {
        let pool = PgPool::connect(&self.database_url).await?;
        let mut created = 0;
        let mut updated = 0;

        for agent in &export.agents {
            let (c, u) = upsert_agent(&pool, agent).await?;
            created += c;
            updated += u;
        }

        for skill in &export.skills {
            let (c, u) = upsert_skill(&pool, skill).await?;
            created += c;
            updated += u;
        }

        for context in &export.contexts {
            let (c, u) = upsert_context(&pool, context).await?;
            created += c;
            updated += u;
        }

        Ok(ImportResult {
            created,
            updated,
            skipped: 0,
        })
    }
}

async fn upsert_agent(pool: &PgPool, agent: &AgentExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query(
        "INSERT INTO agents (id, name, system_prompt, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           system_prompt = EXCLUDED.system_prompt,
           updated_at = EXCLUDED.updated_at",
    )
    .bind(agent.id)
    .bind(&agent.name)
    .bind(&agent.system_prompt)
    .bind(agent.created_at)
    .bind(agent.updated_at)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && agent.created_at == agent.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}

async fn upsert_skill(pool: &PgPool, skill: &SkillExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query(
        "INSERT INTO agent_skills (id, agent_id, name, description, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           description = EXCLUDED.description,
           updated_at = EXCLUDED.updated_at",
    )
    .bind(skill.id)
    .bind(skill.agent_id)
    .bind(&skill.name)
    .bind(&skill.description)
    .bind(skill.created_at)
    .bind(skill.updated_at)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && skill.created_at == skill.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}

async fn upsert_context(pool: &PgPool, context: &ContextExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query(
        "INSERT INTO contexts (id, name, description, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           description = EXCLUDED.description,
           updated_at = EXCLUDED.updated_at",
    )
    .bind(context.id)
    .bind(&context.name)
    .bind(&context.description)
    .bind(context.created_at)
    .bind(context.updated_at)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && context.created_at == context.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}
