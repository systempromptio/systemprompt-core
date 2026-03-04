use crate::models::Agent;
use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
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
        sqlx::query(
            "INSERT INTO agents (agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
             $18, $19, $20)",
        )
        .bind(agent.agent_id.as_str())
        .bind(&agent.name)
        .bind(&agent.display_name)
        .bind(&agent.description)
        .bind(&agent.version)
        .bind(&agent.system_prompt)
        .bind(agent.enabled)
        .bind(agent.port)
        .bind(&agent.endpoint)
        .bind(agent.dev_only)
        .bind(agent.is_primary)
        .bind(agent.is_default)
        .bind(&agent.tags)
        .bind(agent.category_id.as_ref().map(|c| c.to_string()))
        .bind(agent.source_id.as_str())
        .bind(&agent.provider)
        .bind(&agent.model)
        .bind(&agent.mcp_servers)
        .bind(&agent.skills)
        .bind(&agent.card_json)
        .execute(self.write_pool.as_ref())
        .await
        .context(format!("Failed to create agent: {}", agent.name))?;

        Ok(())
    }

    pub async fn get_by_agent_id(&self, agent_id: &AgentId) -> Result<Option<Agent>> {
        let row = sqlx::query(
            "SELECT agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json,
             created_at, updated_at
             FROM agents WHERE agent_id = $1",
        )
        .bind(agent_id.as_str())
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to get agent by id: {agent_id}"))?;

        row.map(|r| agent_from_row(&r)).transpose()
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<Agent>> {
        let row = sqlx::query(
            "SELECT agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json,
             created_at, updated_at
             FROM agents WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to get agent by name: {name}"))?;

        row.map(|r| agent_from_row(&r)).transpose()
    }

    pub async fn list_enabled(&self) -> Result<Vec<Agent>> {
        let rows = sqlx::query(
            "SELECT agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json,
             created_at, updated_at
             FROM agents WHERE enabled = true ORDER BY name ASC",
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list enabled agents")?;

        rows.iter().map(agent_from_row).collect::<Result<Vec<_>>>()
    }

    pub async fn list_all(&self) -> Result<Vec<Agent>> {
        let rows = sqlx::query(
            "SELECT agent_id, name, display_name, description, version,
             system_prompt, enabled, port, endpoint, dev_only, is_primary, is_default,
             tags, category_id, source_id, provider, model, mcp_servers, skills, card_json,
             created_at, updated_at
             FROM agents ORDER BY name ASC",
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list all agents")?;

        rows.iter().map(agent_from_row).collect::<Result<Vec<_>>>()
    }

    pub async fn update(&self, agent_id: &AgentId, agent: &Agent) -> Result<()> {
        sqlx::query(
            "UPDATE agents SET name = $1, display_name = $2, description = $3, version = $4,
             system_prompt = $5, enabled = $6, port = $7, endpoint = $8, dev_only = $9,
             is_primary = $10, is_default = $11, tags = $12, provider = $13, model = $14,
             mcp_servers = $15, skills = $16, card_json = $17, updated_at = CURRENT_TIMESTAMP
             WHERE agent_id = $18",
        )
        .bind(&agent.name)
        .bind(&agent.display_name)
        .bind(&agent.description)
        .bind(&agent.version)
        .bind(&agent.system_prompt)
        .bind(agent.enabled)
        .bind(agent.port)
        .bind(&agent.endpoint)
        .bind(agent.dev_only)
        .bind(agent.is_primary)
        .bind(agent.is_default)
        .bind(&agent.tags)
        .bind(&agent.provider)
        .bind(&agent.model)
        .bind(&agent.mcp_servers)
        .bind(&agent.skills)
        .bind(&agent.card_json)
        .bind(agent_id.as_str())
        .execute(self.write_pool.as_ref())
        .await
        .context(format!("Failed to update agent: {}", agent.name))?;

        Ok(())
    }

    pub async fn delete(&self, agent_id: &AgentId) -> Result<()> {
        sqlx::query("DELETE FROM agents WHERE agent_id = $1")
            .bind(agent_id.as_str())
            .execute(self.write_pool.as_ref())
            .await
            .context(format!("Failed to delete agent: {agent_id}"))?;

        Ok(())
    }
}

fn agent_from_row(row: &sqlx::postgres::PgRow) -> Result<Agent> {
    Ok(Agent {
        agent_id: AgentId::new(row.try_get::<String, _>("agent_id")?.as_str()),
        name: row.try_get("name")?,
        display_name: row.try_get("display_name")?,
        description: row.try_get("description")?,
        version: row.try_get("version")?,
        system_prompt: row.try_get("system_prompt")?,
        enabled: row.try_get("enabled")?,
        port: row.try_get("port")?,
        endpoint: row.try_get("endpoint")?,
        dev_only: row.try_get("dev_only")?,
        is_primary: row.try_get("is_primary")?,
        is_default: row.try_get("is_default")?,
        tags: row
            .try_get::<Option<Vec<String>>, _>("tags")?
            .unwrap_or_else(Vec::new),
        category_id: row
            .try_get::<Option<String>, _>("category_id")?
            .map(|s| CategoryId::new(&s)),
        source_id: SourceId::new(row.try_get::<String, _>("source_id")?.as_str()),
        provider: row.try_get("provider")?,
        model: row.try_get("model")?,
        mcp_servers: row
            .try_get::<Option<Vec<String>>, _>("mcp_servers")?
            .unwrap_or_else(Vec::new),
        skills: row
            .try_get::<Option<Vec<String>>, _>("skills")?
            .unwrap_or_else(Vec::new),
        card_json: row.try_get("card_json")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
