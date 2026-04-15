use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use systemprompt_cloud::ProjectContext;
use systemprompt_identifiers::TenantId;

pub const SHARED_CONTAINER_NAME: &str = "systemprompt-postgres-shared";
pub const SHARED_ADMIN_USER: &str = "systemprompt_admin";
pub const SHARED_VOLUME_NAME: &str = "systemprompt-postgres-shared-data";
pub const SHARED_PORT: u16 = 5432;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedContainerConfig {
    pub admin_password: String,
    pub port: u16,
    pub created_at: DateTime<Utc>,
    pub tenant_databases: Vec<TenantDatabaseMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantDatabaseMapping {
    pub tenant_id: TenantId,
    pub database_name: String,
}

impl SharedContainerConfig {
    pub fn new(admin_password: String, port: u16) -> Self {
        Self {
            admin_password,
            port,
            created_at: Utc::now(),
            tenant_databases: Vec::new(),
        }
    }

    pub fn add_tenant(&mut self, tenant: TenantId, database_name: String) {
        self.tenant_databases.push(TenantDatabaseMapping {
            tenant_id: tenant,
            database_name,
        });
    }

    pub fn remove_tenant(&mut self, tenant: &str) -> Option<TenantDatabaseMapping> {
        self.tenant_databases
            .iter()
            .position(|t| t.tenant_id == tenant)
            .map(|pos| self.tenant_databases.remove(pos))
    }
}

pub fn shared_config_path() -> PathBuf {
    let ctx = ProjectContext::discover();
    ctx.docker_dir().join("shared_config.json")
}

pub fn load_shared_config() -> Result<Option<SharedContainerConfig>> {
    let path = shared_config_path();
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: SharedContainerConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(config))
}

pub fn save_shared_config(config: &SharedContainerConfig) -> Result<()> {
    let path = shared_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}
