//! Tests for `ExtensionRegistry` validation in `registry/validation.rs`:
//! dependency validation (`validate_dependencies`) and API base-path checks
//! (`validate_api_paths`), plus the exported `RESERVED_PATHS` table.

use std::sync::Arc;

use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    LoaderError, RESERVED_PATHS,
};
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

struct RouterExt {
    id: &'static str,
    base_path: Option<&'static str>,
    deps: Vec<&'static str>,
}

impl RouterExt {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            base_path: None,
            deps: Vec::new(),
        }
    }

    fn with_base_path(mut self, path: &'static str) -> Self {
        self.base_path = Some(path);
        self
    }

    fn with_deps(mut self, deps: Vec<&'static str>) -> Self {
        self.deps = deps;
        self
    }
}

impl Extension for RouterExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Router Ext",
            version: "1.0.0",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        self.base_path
            .map(|p| ExtensionRouter::new(axum::Router::new(), p))
    }

    fn dependencies(&self) -> Vec<&'static str> {
        self.deps.clone()
    }
}

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
    fn services_root(&self) -> std::path::PathBuf {
        systemprompt_test_fixtures::test_boot_services_root()
    }
    fn token_authority(&self) -> Arc<dyn systemprompt_models::auth::TokenAuthority> {
        systemprompt_test_fixtures::test_token_authority(&self.app_config().jwt_issuer)
    }
    fn get_extension(&self, _id: &str) -> Option<Arc<dyn Extension>> {
        None
    }
}

fn registry_with(exts: Vec<Arc<dyn Extension>>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.merge(exts).expect("merge stub extensions");
    registry
}

#[test]
fn validate_dependencies_ok_when_satisfied() {
    let registry = registry_with(vec![
        Arc::new(RouterExt::new("base")),
        Arc::new(RouterExt::new("child").with_deps(vec!["base"])),
    ]);
    registry
        .validate_dependencies()
        .expect("satisfied dependency should validate");
}

#[test]
fn merge_refuses_a_missing_dependency() {
    let mut registry = ExtensionRegistry::new();
    let err = registry
        .merge(vec![Arc::new(
            RouterExt::new("needy").with_deps(vec!["absent"]),
        )])
        .expect_err("missing dependency must fail at insert time");
    match err {
        LoaderError::MissingDependency {
            extension,
            dependency,
        } => {
            assert_eq!(extension, "needy");
            assert_eq!(dependency, "absent");
        },
        other => panic!("expected MissingDependency, got {other:?}"),
    }
}

#[test]
fn validate_api_paths_accepts_valid_unreserved_path() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("ok").with_base_path("/api/v1/widgets"),
    )]);
    registry
        .validate_api_paths(&StubCtx)
        .expect("a non-reserved /api/ path is valid");
}

#[test]
fn validate_api_paths_ignores_extensions_without_a_router() {
    let registry = registry_with(vec![Arc::new(RouterExt::new("no-router"))]);
    registry
        .validate_api_paths(&StubCtx)
        .expect("extensions without a router impose no path constraint");
}

#[test]
fn validate_api_paths_rejects_non_api_prefix() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("bad").with_base_path("/widgets"),
    )]);

    let err = registry
        .validate_api_paths(&StubCtx)
        .expect_err("base path outside /api/ must fail");
    match err {
        LoaderError::InvalidBasePath { extension, path } => {
            assert_eq!(extension, "bad");
            assert_eq!(path, "/widgets");
        },
        other => panic!("expected InvalidBasePath, got {other:?}"),
    }
}

#[test]
fn validate_api_paths_accepts_the_web_root() {
    let registry = registry_with(vec![Arc::new(RouterExt::new("site").with_base_path("/"))]);
    registry
        .validate_api_paths(&StubCtx)
        .expect("the web root is the one sanctioned base outside /api/");
}

#[test]
fn validate_api_paths_rejects_reserved_path_collision() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("colliding").with_base_path("/api/v1/oauth/extra"),
    )]);

    let err = registry
        .validate_api_paths(&StubCtx)
        .expect_err("a path under a reserved prefix must fail");
    match err {
        LoaderError::ReservedPathCollision { extension, path } => {
            assert_eq!(extension, "colliding");
            assert_eq!(path, "/api/v1/oauth/extra");
        },
        other => panic!("expected ReservedPathCollision, got {other:?}"),
    }
}

#[test]
fn reserved_paths_table_covers_known_core_surfaces() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/oauth"));
    assert!(RESERVED_PATHS.contains(&"/api/v1/mcp"));
    assert!(RESERVED_PATHS.contains(&"/.well-known"));
    // A bespoke extension path is not reserved.
    assert!(!RESERVED_PATHS.contains(&"/api/v1/widgets"));
}
