pub mod api_client;
pub mod crate_deploy;
pub mod database;
pub mod diff;
pub mod error;
pub mod export;
pub mod files;
pub mod local;
pub mod models;

use serde::{Deserialize, Serialize};

pub use api_client::SyncApiClient;
pub use database::{ContextExport, DatabaseExport, DatabaseSyncService, SkillExport};
pub use diff::{compute_content_hash, ContentDiffCalculator, SkillsDiffCalculator};
pub use error::{SyncError, SyncResult};
pub use export::{
    escape_yaml, export_content_to_file, export_skill_to_disk, generate_content_markdown,
    generate_skill_config, generate_skill_markdown,
};
pub use files::{FileBundle, FileEntry, FileManifest, FileSyncService};
pub use local::{ContentDiffEntry, ContentLocalSync, SkillsLocalSync};
pub use models::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, DiskSkill, LocalSyncDirection,
    LocalSyncResult, SkillDiffItem, SkillsDiffResult,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    Push,
    Pull,
}

#[derive(Clone, Debug)]
pub struct SyncConfig {
    pub direction: SyncDirection,
    pub dry_run: bool,
    pub verbose: bool,
    pub tenant_id: String,
    pub api_url: String,
    pub api_token: String,
    pub services_path: String,
    pub hostname: Option<String>,
    pub sync_token: Option<String>,
    pub local_database_url: Option<String>,
}

#[derive(Debug)]
pub struct SyncConfigBuilder {
    direction: SyncDirection,
    dry_run: bool,
    verbose: bool,
    tenant_id: String,
    api_url: String,
    api_token: String,
    services_path: String,
    hostname: Option<String>,
    sync_token: Option<String>,
    local_database_url: Option<String>,
}

impl SyncConfigBuilder {
    pub fn new(
        tenant_id: impl Into<String>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> Self {
        Self {
            direction: SyncDirection::Push,
            dry_run: false,
            verbose: false,
            tenant_id: tenant_id.into(),
            api_url: api_url.into(),
            api_token: api_token.into(),
            services_path: services_path.into(),
            hostname: None,
            sync_token: None,
            local_database_url: None,
        }
    }

    pub const fn with_direction(mut self, direction: SyncDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_hostname(mut self, hostname: Option<String>) -> Self {
        self.hostname = hostname;
        self
    }

    pub fn with_sync_token(mut self, sync_token: Option<String>) -> Self {
        self.sync_token = sync_token;
        self
    }

    pub fn with_local_database_url(mut self, url: impl Into<String>) -> Self {
        self.local_database_url = Some(url.into());
        self
    }

    pub fn build(self) -> SyncConfig {
        SyncConfig {
            direction: self.direction,
            dry_run: self.dry_run,
            verbose: self.verbose,
            tenant_id: self.tenant_id,
            api_url: self.api_url,
            api_token: self.api_token,
            services_path: self.services_path,
            hostname: self.hostname,
            sync_token: self.sync_token,
            local_database_url: self.local_database_url,
        }
    }
}

impl SyncConfig {
    pub fn builder(
        tenant_id: impl Into<String>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> SyncConfigBuilder {
        SyncConfigBuilder::new(tenant_id, api_url, api_token, services_path)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncOperationResult {
    pub operation: String,
    pub success: bool,
    pub items_synced: usize,
    pub items_skipped: usize,
    pub errors: Vec<String>,
    pub details: Option<serde_json::Value>,
}

impl SyncOperationResult {
    pub fn success(operation: &str, items_synced: usize) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced,
            items_skipped: 0,
            errors: vec![],
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn dry_run(operation: &str, items_skipped: usize, details: serde_json::Value) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced: 0,
            items_skipped,
            errors: vec![],
            details: Some(details),
        }
    }
}

#[derive(Debug)]
pub struct SyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl SyncService {
    pub fn new(config: SyncConfig) -> Self {
        let api_client = SyncApiClient::new(&config.api_url, &config.api_token)
            .with_direct_sync(config.hostname.clone(), config.sync_token.clone());
        Self { config, api_client }
    }

    pub async fn sync_files(&self) -> SyncResult<SyncOperationResult> {
        let service = FileSyncService::new(self.config.clone(), self.api_client.clone());
        service.sync().await
    }

    pub async fn sync_database(&self) -> SyncResult<SyncOperationResult> {
        let local_db_url = self
            .config
            .local_database_url
            .as_ref()
            .ok_or_else(|| SyncError::MissingConfig("local_database_url not configured".to_string()))?;

        let cloud_db_url = self
            .api_client
            .get_database_url(&self.config.tenant_id)
            .await
            .map_err(|e| SyncError::ApiError {
                status: 500,
                message: format!("Failed to get cloud database URL: {e}"),
            })?;

        let service = DatabaseSyncService::new(
            self.config.direction,
            self.config.dry_run,
            local_db_url,
            &cloud_db_url,
        );

        service.sync().await
    }

    pub async fn sync_all(&self) -> SyncResult<Vec<SyncOperationResult>> {
        let mut results = Vec::new();

        let files_result = self.sync_files().await?;
        results.push(files_result);

        match self.sync_database().await {
            Ok(db_result) => results.push(db_result),
            Err(e) => {
                tracing::warn!(error = %e, "Database sync failed");
                results.push(SyncOperationResult {
                    operation: "database".to_string(),
                    success: false,
                    items_synced: 0,
                    items_skipped: 0,
                    errors: vec![e.to_string()],
                    details: None,
                });
            },
        }

        Ok(results)
    }
}
