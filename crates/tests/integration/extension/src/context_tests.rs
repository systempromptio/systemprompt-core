//! Tests for ExtensionContext trait.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use std::sync::Mutex;
use systemprompt_extension::context::{DynExtensionContext, ExtensionContext};
use systemprompt_extension::{Extension, ExtensionMetadata};

use systemprompt_identifiers::InstanceId;
use systemprompt_traits::{
    ByteRange, ConfigProvider, DatabaseHandle, FileStorage, FileStorageError, FileStorageResult,
    StoredFileId, StoredFileMetadata, StoredFileRead,
};

#[derive(Debug, Clone)]
struct MockConfig;

impl ConfigProvider for MockConfig {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "test_key" => Some("test_value".to_string()),
            _ => None,
        }
    }

    fn database_url(&self) -> &str {
        "postgres://test:test@localhost/test"
    }

    fn system_path(&self) -> &str {
        "/tmp/test-system"
    }

    fn api_port(&self) -> u16 {
        3000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct MockDatabase {
    connected: bool,
}

impl DatabaseHandle for MockDatabase {
    fn is_connected(&self) -> bool {
        self.connected
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct TestExtension {
    id: &'static str,
}

impl Extension for TestExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Test Extension",
            version: "1.0.0",
        }
    }
}

#[derive(Debug, Default)]
struct MemoryFiles {
    objects: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryFiles {
    fn bytes_of(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
        self.objects
            .lock()
            .expect("lock")
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| FileStorageError::NotFound(id.to_string()))
    }
}

#[async_trait::async_trait]
impl FileStorage for MemoryFiles {
    async fn store(
        &self,
        path: &std::path::Path,
        content: &[u8],
    ) -> FileStorageResult<StoredFileId> {
        let key = path.display().to_string();
        self.objects
            .lock()
            .expect("lock")
            .insert(key.clone(), content.to_vec());
        Ok(StoredFileId::new(key))
    }

    async fn open(
        &self,
        id: &StoredFileId,
        range: Option<ByteRange>,
    ) -> FileStorageResult<StoredFileRead> {
        let content = self.bytes_of(id)?;
        let metadata = self.metadata(id).await?;
        let served = range.map(|r| r.resolve(content.len() as u64)).transpose()?;
        let body = served.map_or_else(
            || content.clone(),
            |r| content[r.start as usize..=r.end as usize].to_vec(),
        );
        let chunk: FileStorageResult<bytes::Bytes> = Ok(bytes::Bytes::from(body));
        Ok(StoredFileRead {
            metadata,
            range: served,
            body: Box::pin(futures::stream::iter([chunk])),
        })
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        self.objects.lock().expect("lock").remove(id.as_str());
        Ok(())
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let size = self.bytes_of(id)?.len();
        Ok(StoredFileMetadata {
            id: id.clone(),
            path: id.to_string(),
            mime_type: "application/octet-stream".to_string(),
            etag: None,
            size_bytes: i64::try_from(size).ok(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        Ok(self.objects.lock().expect("lock").contains_key(id.as_str()))
    }
}

struct MockExtensionContext {
    config: Arc<dyn ConfigProvider>,
    database: Arc<dyn DatabaseHandle>,
    extensions: HashMap<String, Arc<dyn Extension>>,
    files: Arc<MemoryFiles>,
    instance: InstanceId,
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

impl ExtensionContext for MockExtensionContext {
    fn subject_attributes(&self) -> Arc<dyn systemprompt_models::authz::SubjectAttributeSource> {
        Arc::new(StubSubjects)
    }
    fn provider_tokens(&self) -> Arc<dyn systemprompt_traits::ProviderTokenCache> {
        Arc::new(StubTokens)
    }
    fn secrets(&self) -> Arc<dyn systemprompt_traits::SecretStore> {
        systemprompt_test_fixtures::fixture_secret_store()
    }
    fn services(&self) -> systemprompt_manifest::services::ServicesView {
        systemprompt_test_fixtures::fixture_services_view()
    }
    fn system_owner_id(&self) -> systemprompt_identifiers::UserId {
        systemprompt_identifiers::UserId::new("extension-test-owner")
    }
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::clone(&self.config)
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::clone(&self.database)
    }

    fn file_storage(&self) -> Arc<dyn FileStorage> {
        let files: Arc<dyn FileStorage> = Arc::clone(&self.files) as Arc<dyn FileStorage>;
        files
    }

    fn instance_id(&self) -> InstanceId {
        self.instance.clone()
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
    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extensions.get(id).cloned()
    }
}

#[test]
fn test_extension_context_config() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let config = ctx.config();
    assert_eq!(config.get("test_key"), Some("test_value".to_string()));
    assert_eq!(config.database_url(), "postgres://test:test@localhost/test");
    assert_eq!(config.api_port(), 3000);
}

#[test]
fn test_extension_context_database() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let db = ctx.database();
    assert!(db.is_connected());
}

#[test]
fn test_extension_context_database_disconnected() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: false }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let db = ctx.database();
    assert!(!db.is_connected());
}

#[test]
fn test_extension_context_get_extension_exists() {
    let ext: Arc<dyn Extension> = Arc::new(TestExtension { id: "test-ext" });
    let mut extensions = HashMap::new();
    extensions.insert("test-ext".to_string(), ext);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let found = ctx.get_extension("test-ext");
    found.as_ref().expect("found should be present");
    assert_eq!(found.expect("extension exists").id(), "test-ext");
}

#[test]
fn test_extension_context_get_extension_not_found() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    assert!(ctx.get_extension("nonexistent").is_none());
}

#[test]
fn test_extension_context_has_extension_true() {
    let ext: Arc<dyn Extension> = Arc::new(TestExtension { id: "my-ext" });
    let mut extensions = HashMap::new();
    extensions.insert("my-ext".to_string(), ext);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    assert!(ctx.has_extension("my-ext"));
}

#[test]
fn test_extension_context_has_extension_false() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    assert!(!ctx.has_extension("missing-ext"));
}

#[test]
fn test_extension_context_multiple_extensions() {
    let ext1: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-1" });
    let ext2: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-2" });
    let ext3: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-3" });

    let mut extensions = HashMap::new();
    extensions.insert("ext-1".to_string(), ext1);
    extensions.insert("ext-2".to_string(), ext2);
    extensions.insert("ext-3".to_string(), ext3);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    assert!(ctx.has_extension("ext-1"));
    assert!(ctx.has_extension("ext-2"));
    assert!(ctx.has_extension("ext-3"));
    assert!(!ctx.has_extension("ext-4"));

    assert_eq!(
        ctx.get_extension("ext-1").expect("ext-1 exists").id(),
        "ext-1"
    );
    assert_eq!(
        ctx.get_extension("ext-2").expect("ext-2 exists").id(),
        "ext-2"
    );
}

#[test]
fn test_dyn_extension_context_type_alias() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    assert!(dyn_ctx.database().is_connected());
}

#[test]
fn test_dyn_extension_context_clone() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    let cloned = Arc::clone(&dyn_ctx);

    assert!(dyn_ctx.database().is_connected());
    assert!(cloned.database().is_connected());
}

#[test]
fn test_dyn_extension_context_thread_safe() {
    use std::thread;

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    let ctx_clone = Arc::clone(&dyn_ctx);

    let handle = thread::spawn(move || ctx_clone.database().is_connected());

    assert!(dyn_ctx.database().is_connected());
    assert!(handle.join().expect("thread should complete"));
}

#[test]
fn test_extension_default_schemas() {
    let ext = TestExtension { id: "test" };
    assert!(ext.schemas().is_empty());
}

#[test]
fn test_extension_default_router() {
    let ext = TestExtension { id: "test" };
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };
    assert!(ext.router(&ctx).is_none());
}

#[test]
fn test_extension_default_jobs() {
    let ext = TestExtension { id: "test" };
    assert!(ext.jobs().is_empty());
}

#[test]
fn test_extension_default_config_prefix() {
    let ext = TestExtension { id: "test" };
    assert!(ext.config_prefix().is_none());
}

#[test]
fn test_extension_default_config_schema() {
    let ext = TestExtension { id: "test" };
    assert!(ext.config_schema().is_none());
}

#[test]
fn test_extension_default_validate_config() {
    let ext = TestExtension { id: "test" };
    let config = serde_json::json!({});
    ext.validate_config(&config)
        .expect("ext.validate_config(&config) should succeed");
}

#[test]
fn test_extension_default_tool_providers() {
    let ext = TestExtension { id: "test" };
    assert!(ext.tool_providers().is_empty());
}

#[test]
fn test_extension_default_dependencies() {
    let ext = TestExtension { id: "test" };
    assert!(ext.dependencies().is_empty());
}

#[test]
fn test_extension_default_priority() {
    let ext = TestExtension { id: "test" };
    assert_eq!(ext.priority(), 100);
}

#[test]
fn test_extension_derived_methods() {
    let ext = TestExtension { id: "my-ext" };

    assert_eq!(ext.id(), "my-ext");
    assert_eq!(ext.name(), "Test Extension");
    assert_eq!(ext.version(), "1.0.0");
}

#[test]
fn test_extension_has_methods() {
    let ext = TestExtension { id: "test" };
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    };

    assert!(!ext.has_schemas());
    assert!(ext.router(&ctx).is_none());
    assert!(!ext.has_jobs());
    assert!(!ext.has_config());
    assert!(!ext.has_tool_providers());
}

#[tokio::test]
async fn file_storage_shares_one_backend_across_handles() {
    let files = Arc::new(MemoryFiles::default());
    let ctx: DynExtensionContext = Arc::new(MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::clone(&files),
        instance: InstanceId::new("replica-a"),
    });

    let id = ctx
        .file_storage()
        .store(std::path::Path::new("exports/a.json"), b"payload")
        .await
        .expect("store");
    let read = ctx.file_storage().open(&id, None).await.expect("open");
    let chunks: Vec<_> = futures::StreamExt::collect(read.body).await;
    let body: Vec<u8> = chunks
        .into_iter()
        .flat_map(|chunk| chunk.expect("chunk").to_vec())
        .collect();
    assert_eq!(body, b"payload");
    assert!(files.exists(&id).await.expect("exists"));
}

#[test]
fn instance_id_is_the_context_replica() {
    let ctx: DynExtensionContext = Arc::new(MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
        files: Arc::new(MemoryFiles::default()),
        instance: InstanceId::new("replica-a"),
    });
    assert_eq!(ctx.instance_id().as_str(), "replica-a");
}
