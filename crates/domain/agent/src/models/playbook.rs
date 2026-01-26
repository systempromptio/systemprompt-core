use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_database::JsonRow;
use systemprompt_identifiers::{PlaybookId, SourceId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub playbook_id: PlaybookId,
    pub file_path: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub category: String,
    pub domain: String,
    pub source_id: SourceId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Playbook {
    pub fn from_json_row(row: &JsonRow) -> Result<Self> {
        let playbook_id = PlaybookId::new(
            row.get("playbook_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing playbook_id"))?,
        );

        let file_path = row
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing file_path"))?
            .to_string();

        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing name"))?
            .to_string();

        let description = row
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing description"))?
            .to_string();

        let instructions = row
            .get("instructions")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing instructions"))?
            .to_string();

        let enabled = row
            .get("enabled")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow!("Missing enabled"))?;

        let tags = row
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let category = row
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing category"))?
            .to_string();

        let domain = row
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing domain"))?
            .to_string();

        let source_id = SourceId::new(
            row.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing source_id"))?,
        );

        let created_at = row
            .get("created_at")
            .and_then(|v| systemprompt_database::parse_database_datetime(v))
            .ok_or_else(|| anyhow!("Missing or invalid created_at"))?;

        let updated_at = row
            .get("updated_at")
            .and_then(|v| systemprompt_database::parse_database_datetime(v))
            .ok_or_else(|| anyhow!("Missing or invalid updated_at"))?;

        Ok(Self {
            playbook_id,
            file_path,
            name,
            description,
            instructions,
            enabled,
            tags,
            category,
            domain,
            source_id,
            created_at,
            updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub file: String,
    pub category: String,
    pub domain: String,
    pub tags: Vec<String>,
}
