use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_database::JsonRow;
use systemprompt_identifiers::{AgentId, CategoryId, SourceId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub agent_id: AgentId,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub system_prompt: Option<String>,
    pub enabled: bool,
    pub port: i32,
    pub endpoint: String,
    pub dev_only: bool,
    pub is_primary: bool,
    pub is_default: bool,
    pub tags: Vec<String>,
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
    pub card_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Agent {
    pub fn from_json_row(row: &JsonRow) -> Result<Self> {
        let agent_id = AgentId::new(
            row.get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing agent_id"))?,
        );

        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing name"))?
            .to_string();

        let display_name = row
            .get("display_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing display_name"))?
            .to_string();

        let description = row
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing description"))?
            .to_string();

        let version = row
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing version"))?
            .to_string();

        let system_prompt = row
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .map(String::from);

        let enabled = row
            .get("enabled")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow!("Missing enabled"))?;

        let port = row
            .get("port")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("Missing port"))? as i32;

        let endpoint = row
            .get("endpoint")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing endpoint"))?
            .to_string();

        let dev_only = row
            .get("dev_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let is_primary = row
            .get("is_primary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let is_default = row
            .get("is_default")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tags = row
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let category_id = row
            .get("category_id")
            .and_then(|v| v.as_str())
            .map(CategoryId::new);

        let source_id = SourceId::new(
            row.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing source_id"))?,
        );

        let provider = row
            .get("provider")
            .and_then(|v| v.as_str())
            .map(String::from);

        let model = row.get("model").and_then(|v| v.as_str()).map(String::from);

        let mcp_servers = row
            .get("mcp_servers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let skills = row
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let card_json = row
            .get("card_json")
            .cloned()
            .ok_or_else(|| anyhow!("Missing card_json"))?;

        let created_at = row
            .get("created_at")
            .and_then(|v| systemprompt_database::parse_database_datetime(v))
            .ok_or_else(|| anyhow!("Missing or invalid created_at"))?;

        let updated_at = row
            .get("updated_at")
            .and_then(|v| systemprompt_database::parse_database_datetime(v))
            .ok_or_else(|| anyhow!("Missing or invalid updated_at"))?;

        Ok(Self {
            agent_id,
            name,
            display_name,
            description,
            version,
            system_prompt,
            enabled,
            port,
            endpoint,
            dev_only,
            is_primary,
            is_default,
            tags,
            category_id,
            source_id,
            provider,
            model,
            mcp_servers,
            skills,
            card_json,
            created_at,
            updated_at,
        })
    }
}
