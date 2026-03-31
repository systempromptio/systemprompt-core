use crate::models::{Agent, AgentRow};
use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentId, CategoryId, SourceId};

#[derive(Debug)]
pub struct AgentRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl AgentRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc().context("PostgreSQL pool not available")?;
        let write_pool = db
            .write_pool_arc()
            .context("Write PostgreSQL pool not available")?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create(&self, agent: &Agent) -> Result<()> {
        let agent_id_str = agent.id.as_str();
        let category_id = agent.category_id.as_ref().map(ToString::to_string);
        let source_id_str = agent.source_id.as_str();

        sqlx::query!(
            "INSERT INTO agents (agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
             $18, $19, $20)",
            agent_id_str,
            agent.name,
            agent.display_name,
            agent.description,
            agent.version,
            agent.system_prompt,
            agent.enabled,
            agent.port,
            agent.endpoint,
            agent.dev_only,
            agent.is_primary,
            agent.is_default,
            &agent.tags[..],
            category_id,
            source_id_str,
            agent.provider,
            agent.model,
            &agent.mcp_servers[..],
            &agent.skills[..],
            agent.card_json
        )
        .execute(self.write_pool.as_ref())
        .await
        .context(format!("Failed to create agent: {}", agent.name))?;

        Ok(())
    }

    pub async fn get_by_agent_id(&self, agent_id: &AgentId) -> Result<Option<Agent>> {
        let agent_id_str = agent_id.as_str();

        let row = sqlx::query_as!(
            AgentRow,
            r#"SELECT
                agent_id as "agent_id!: AgentId",
                name as "name!",
                display_name as "display_name!",
                description as "description!",
                version as "version!",
                system_prompt,
                enabled as "enabled!",
                port as "port!",
                endpoint as "endpoint!",
                dev_only as "dev_only!",
                is_primary as "is_primary!",
                is_default as "is_default!",
                tags,
                category_id as "category_id?: CategoryId",
                source_id as "source_id!: SourceId",
                provider,
                model,
                mcp_servers,
                skills,
                card_json as "card_json!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agents WHERE agent_id = $1"#,
            agent_id_str
        )
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to get agent by id: {agent_id}"))?;

        Ok(row.map(agent_from_row))
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<Agent>> {
        let row = sqlx::query_as!(
            AgentRow,
            r#"SELECT
                agent_id as "agent_id!: AgentId",
                name as "name!",
                display_name as "display_name!",
                description as "description!",
                version as "version!",
                system_prompt,
                enabled as "enabled!",
                port as "port!",
                endpoint as "endpoint!",
                dev_only as "dev_only!",
                is_primary as "is_primary!",
                is_default as "is_default!",
                tags,
                category_id as "category_id?: CategoryId",
                source_id as "source_id!: SourceId",
                provider,
                model,
                mcp_servers,
                skills,
                card_json as "card_json!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agents WHERE name = $1"#,
            name
        )
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to get agent by name: {name}"))?;

        Ok(row.map(agent_from_row))
    }

    pub async fn list_enabled(&self) -> Result<Vec<Agent>> {
        let rows = sqlx::query_as!(
            AgentRow,
            r#"SELECT
                agent_id as "agent_id!: AgentId",
                name as "name!",
                display_name as "display_name!",
                description as "description!",
                version as "version!",
                system_prompt,
                enabled as "enabled!",
                port as "port!",
                endpoint as "endpoint!",
                dev_only as "dev_only!",
                is_primary as "is_primary!",
                is_default as "is_default!",
                tags,
                category_id as "category_id?: CategoryId",
                source_id as "source_id!: SourceId",
                provider,
                model,
                mcp_servers,
                skills,
                card_json as "card_json!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agents WHERE enabled = true ORDER BY name ASC"#
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list enabled agents")?;

        Ok(rows.into_iter().map(agent_from_row).collect())
    }

    pub async fn list_all(&self) -> Result<Vec<Agent>> {
        let rows = sqlx::query_as!(
            AgentRow,
            r#"SELECT
                agent_id as "agent_id!: AgentId",
                name as "name!",
                display_name as "display_name!",
                description as "description!",
                version as "version!",
                system_prompt,
                enabled as "enabled!",
                port as "port!",
                endpoint as "endpoint!",
                dev_only as "dev_only!",
                is_primary as "is_primary!",
                is_default as "is_default!",
                tags,
                category_id as "category_id?: CategoryId",
                source_id as "source_id!: SourceId",
                provider,
                model,
                mcp_servers,
                skills,
                card_json as "card_json!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agents ORDER BY name ASC"#
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list all agents")?;

        Ok(rows.into_iter().map(agent_from_row).collect())
    }

    pub async fn update(&self, agent_id: &AgentId, agent: &Agent) -> Result<()> {
        let agent_id_str = agent_id.as_str();

        sqlx::query!(
            "UPDATE agents SET name = $1, display_name = $2, description = $3, version = $4,
             system_prompt = $5, enabled = $6, port = $7, endpoint = $8, dev_only = $9,
             is_primary = $10, is_default = $11, tags = $12, provider = $13, model = $14,
             mcp_servers = $15, skills = $16, card_json = $17, updated_at = CURRENT_TIMESTAMP
             WHERE agent_id = $18",
            agent.name,
            agent.display_name,
            agent.description,
            agent.version,
            agent.system_prompt,
            agent.enabled,
            agent.port,
            agent.endpoint,
            agent.dev_only,
            agent.is_primary,
            agent.is_default,
            &agent.tags[..],
            agent.provider,
            agent.model,
            &agent.mcp_servers[..],
            &agent.skills[..],
            agent.card_json,
            agent_id_str
        )
        .execute(self.write_pool.as_ref())
        .await
        .context(format!("Failed to update agent: {}", agent.name))?;

        Ok(())
    }

    pub async fn delete(&self, agent_id: &AgentId) -> Result<()> {
        let agent_id_str = agent_id.as_str();

        sqlx::query!("DELETE FROM agents WHERE agent_id = $1", agent_id_str)
            .execute(self.write_pool.as_ref())
            .await
            .context(format!("Failed to delete agent: {agent_id}"))?;

        Ok(())
    }
}

fn agent_from_row(row: AgentRow) -> Agent {
    Agent {
        id: row.agent_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        version: row.version,
        system_prompt: row.system_prompt,
        enabled: row.enabled,
        port: row.port,
        endpoint: row.endpoint,
        dev_only: row.dev_only,
        is_primary: row.is_primary,
        is_default: row.is_default,
        tags: row.tags.unwrap_or_else(Vec::new),
        category_id: row.category_id,
        source_id: row.source_id,
        provider: row.provider,
        model: row.model,
        mcp_servers: row.mcp_servers.unwrap_or_else(Vec::new),
        skills: row.skills.unwrap_or_else(Vec::new),
        card_json: row.card_json,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
