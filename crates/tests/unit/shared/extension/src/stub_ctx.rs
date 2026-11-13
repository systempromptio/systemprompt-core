//! The stub `ExtensionContext` shared by the registry tests: every handle is
//! inert, and `extension_state` is empty.

use std::sync::Arc;

use systemprompt_extension::{Extension, ExtensionContext, ExtensionRegistry};
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

pub(crate) struct StubCtx;

#[derive(Debug)]
struct StubConfig;

impl ConfigProvider for StubConfig {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }
    fn database_url(&self) -> &str {
        "postgres://x/y"
    }
    fn system_path(&self) -> &str {
        "/tmp"
    }
    fn api_port(&self) -> u16 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct StubDb;

impl DatabaseHandle for StubDb {
    fn is_connected(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct StubFiles;

#[async_trait::async_trait]
impl systemprompt_traits::FileStorage for StubFiles {
    async fn store(
        &self,
        path: &std::path::Path,
        _content: &[u8],
    ) -> systemprompt_traits::FileStorageResult<systemprompt_traits::StoredFileId> {
        Err(systemprompt_traits::FileStorageError::Validation(
            path.display().to_string(),
        ))
    }
    async fn open(
        &self,
        id: &systemprompt_traits::StoredFileId,
        _range: Option<systemprompt_traits::ByteRange>,
    ) -> systemprompt_traits::FileStorageResult<systemprompt_traits::StoredFileRead> {
        Err(systemprompt_traits::FileStorageError::NotFound(
            id.to_string(),
        ))
    }
    async fn delete(
        &self,
        id: &systemprompt_traits::StoredFileId,
    ) -> systemprompt_traits::FileStorageResult<()> {
        Err(systemprompt_traits::FileStorageError::NotFound(
            id.to_string(),
        ))
    }
    async fn metadata(
        &self,
        id: &systemprompt_traits::StoredFileId,
    ) -> systemprompt_traits::FileStorageResult<systemprompt_traits::StoredFileMetadata> {
        Err(systemprompt_traits::FileStorageError::NotFound(
            id.to_string(),
        ))
    }
    async fn exists(
        &self,
        _id: &systemprompt_traits::StoredFileId,
    ) -> systemprompt_traits::FileStorageResult<bool> {
        Ok(false)
    }
}

#[derive(Debug)]
struct StubTokens;

impl systemprompt_traits::ProviderTokenCache for StubTokens {
    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

struct StubSubjects;

#[async_trait::async_trait]
impl systemprompt_models::authz::SubjectAttributeSource for StubSubjects {
    fn dimensions(&self) -> &[systemprompt_models::authz::SubjectDimension] {
        &[]
    }

    async fn values_for(
        &self,
        _user: &systemprompt_identifiers::UserId,
        dimension: &systemprompt_models::authz::SubjectDimension,
    ) -> Result<Vec<String>, systemprompt_models::authz::SubjectSourceError> {
        Err(
            systemprompt_models::authz::SubjectSourceError::UnknownDimension(
                dimension.rule_type.as_str().to_owned(),
            ),
        )
    }

    async fn resolve_all(
        &self,
        _user: &systemprompt_identifiers::UserId,
    ) -> Result<
        systemprompt_models::authz::SubjectAttributes,
        systemprompt_models::authz::SubjectSourceError,
    > {
        Ok(systemprompt_models::authz::SubjectAttributes::new())
    }
}

impl ExtensionContext for StubCtx {
    fn subject_attributes(&self) -> Arc<dyn systemprompt_models::authz::SubjectAttributeSource> {
        Arc::new(StubSubjects)
    }
    fn provider_tokens(&self) -> Arc<dyn systemprompt_traits::ProviderTokenCache> {
        Arc::new(StubTokens)
    }
    fn secrets(&self) -> Arc<dyn systemprompt_traits::SecretStore> {
        systemprompt_test_fixtures::fixture_secret_store()
    }
    fn dependencies(&self) -> systemprompt_provider_contracts::Dependencies {
        systemprompt_provider_contracts::Dependencies::new()
    }
    fn services(&self) -> systemprompt_manifest::services::ServicesView {
        systemprompt_test_fixtures::fixture_services_view()
    }
    fn system_owner_id(&self) -> systemprompt_identifiers::UserId {
        systemprompt_identifiers::UserId::new("extension-test-owner")
    }
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::new(StubConfig)
    }
    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::new(StubDb)
    }
    fn file_storage(&self) -> Arc<dyn systemprompt_traits::FileStorage> {
        Arc::new(StubFiles)
    }
    fn instance_id(&self) -> systemprompt_identifiers::InstanceId {
        systemprompt_identifiers::InstanceId::new("extension-test-instance")
    }
    fn background_tasks(&self) -> systemprompt_traits::BackgroundTasks {
        systemprompt_traits::BackgroundTasks::new()
    }
    fn profile(&self) -> Arc<systemprompt_manifest::Profile> {
        systemprompt_test_fixtures::test_boot_profile()
    }
    fn app_config(&self) -> Arc<systemprompt_manifest::Config> {
        Arc::clone(systemprompt_test_fixtures::ensure_test_bootstrap().boot_config())
    }
    fn services_root(&self) -> systemprompt_manifest::services::root::ActiveServicesRoot {
        systemprompt_test_fixtures::test_boot_active_services_root()
    }
    fn token_authority(&self) -> Arc<dyn systemprompt_models::auth::TokenAuthority> {
        systemprompt_test_fixtures::test_token_authority(&self.app_config().jwt_issuer)
    }
    fn get_extension(&self, _id: &str) -> Option<Arc<dyn Extension>> {
        None
    }
    fn extension_state(&self) -> systemprompt_extension::ExtensionState {
        systemprompt_extension::ExtensionState::default()
    }
}

pub(crate) fn registry_with(exts: Vec<Arc<dyn Extension>>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.merge(exts).expect("merge stub extensions");
    registry
}
