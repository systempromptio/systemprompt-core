// End-to-end tests for `ImageService` and its `image_persistence` helpers.
//
// `ImageService::generate_image` drives a stub `ImageProvider`, writes the
// decoded bytes to a temp-dir-backed `ImageStorage`, persists an `ai_requests`
// audit row to the migrated test DB, and records the file via a
// `AiFilePersistenceProvider`. We use an in-memory file provider so we can
// assert the persisted rows are queryable through `get_generated_image`,
// `list_user_images`, and `delete_image`. The success path needs the real DB
// (for the audit FK), so each test skips cleanly when DATABASE_URL is unset.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use systemprompt_ai::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageGenerationResponse, ImageResolution,
    NewImageGenerationResponse,
};
use systemprompt_ai::services::providers::{
    BoxedImageProvider, ImageProvider, ImageProviderCapabilities,
};
use systemprompt_ai::{ImageService, StorageConfig};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_traits::{
    AiFilePersistenceProvider, AiGeneratedFile, AiProviderError, AiProviderResult,
    ImageStorageConfig, InsertAiFileParams,
};

const ONE_PIXEL_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(pool)
}

// In-memory file-record store standing in for the database-backed persistence
// provider. Records are keyed by FileId so the read-back assertions in these
// tests observe exactly what `persist_file_record` wrote.
#[derive(Debug, Default)]
struct InMemoryFileProvider {
    files: Mutex<HashMap<String, AiGeneratedFile>>,
}

#[async_trait]
impl AiFilePersistenceProvider for InMemoryFileProvider {
    async fn insert_file(&self, params: InsertAiFileParams) -> AiProviderResult<()> {
        let now = chrono::Utc::now();
        let file = AiGeneratedFile {
            id: params.id,
            path: params.path,
            public_url: params.public_url,
            mime_type: params.mime_type,
            size_bytes: params.size_bytes,
            ai_content: true,
            metadata: params.metadata,
            user_id: params.user_id,
            session_id: params.session_id,
            trace_id: params.trace_id,
            context_id: params.context_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        self.files
            .lock()
            .expect("lock")
            .insert(params.id.to_string(), file);
        Ok(())
    }

    async fn find_by_id(&self, id: &FileId) -> AiProviderResult<Option<AiGeneratedFile>> {
        Ok(self.files.lock().expect("lock").get(id.as_str()).cloned())
    }

    async fn list_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> AiProviderResult<Vec<AiGeneratedFile>> {
        let files = self.files.lock().expect("lock");
        let mut matching: Vec<AiGeneratedFile> = files
            .values()
            .filter(|f| f.user_id.as_ref() == Some(user_id))
            .cloned()
            .collect();
        matching.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(matching
            .into_iter()
            .skip(offset.max(0) as usize)
            .take(limit.max(0) as usize)
            .collect())
    }

    async fn delete(&self, id: &FileId) -> AiProviderResult<()> {
        self.files.lock().expect("lock").remove(id.as_str());
        Ok(())
    }

    fn storage_config(&self) -> AiProviderResult<ImageStorageConfig> {
        Err(AiProviderError::ConfigurationError {
            message: "not used in tests".to_owned(),
        })
    }
}

// Stub image provider that either returns a canned base64 image or fails.
struct StubImageProvider {
    name: String,
    model: String,
    fail: bool,
}

impl StubImageProvider {
    fn ok(name: &str, model: &str) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            fail: false,
        }
    }

    fn failing(name: &str, model: &str) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            fail: true,
        }
    }
}

#[async_trait]
impl ImageProvider for StubImageProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ImageProviderCapabilities {
        ImageProviderCapabilities {
            supported_resolutions: vec![ImageResolution::OneK],
            supported_aspect_ratios: vec![AspectRatio::Square],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 1000,
            cost_per_image_cents: 4.0,
        }
    }

    fn supported_models(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    fn default_model(&self) -> &str {
        &self.model
    }

    async fn generate_image(
        &self,
        request: &ImageGenerationRequest,
    ) -> systemprompt_ai::error::Result<ImageGenerationResponse> {
        if self.fail {
            return Err(systemprompt_ai::error::AiError::ProviderError {
                provider: self.name.clone(),
                message: "stub image failure".to_owned(),
            });
        }
        Ok(ImageGenerationResponse::new(NewImageGenerationResponse {
            provider: self.name.clone(),
            model: self.model.clone(),
            image_data: ONE_PIXEL_PNG_BASE64.to_owned(),
            mime_type: "image/png".to_owned(),
            resolution: request.resolution,
            aspect_ratio: request.aspect_ratio,
            generation_time_ms: 42,
        }))
    }
}

fn storage_config() -> (tempfile::TempDir, StorageConfig) {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = StorageConfig::new(dir.path().join("images"), "/media".to_owned());
    (dir, config)
}

fn build_service(
    pool: &DbPool,
    file_provider: Arc<InMemoryFileProvider>,
    providers: Vec<(String, BoxedImageProvider)>,
    default: Option<String>,
) -> (tempfile::TempDir, ImageService) {
    let (dir, config) = storage_config();
    let mut map: HashMap<String, BoxedImageProvider> = HashMap::new();
    for (name, p) in providers {
        map.insert(name, p);
    }
    let service = ImageService::with_providers(pool, config, file_provider, map, default)
        .expect("image service builds");
    (dir, service)
}

fn request(user_id: &UserId, model: Option<&str>) -> ImageGenerationRequest {
    ImageGenerationRequest {
        prompt: "a small red pixel".to_owned(),
        model: model.map(ToOwned::to_owned),
        resolution: ImageResolution::OneK,
        aspect_ratio: AspectRatio::Square,
        reference_images: Vec::new(),
        enable_search_grounding: false,
        user_id: user_id.clone(),
        session_id: None,
        trace_id: None,
        mcp_execution_id: None,
    }
}

async fn seed_user(pool: &DbPool) -> UserId {
    let user_id = unique_user_id("ai-image");
    let email = format!("{}@ai-image.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.expect("seed");
    user_id
}

#[tokio::test]
async fn generate_image_persists_file_and_audit_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        Arc::clone(&file_provider),
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let response = service
        .generate_image(request(&user_id, None))
        .await
        .expect("generate ok");

    assert_eq!(response.provider, "stub");
    assert_eq!(response.model, "stub-image-1");
    assert!(response.file_path.is_some());
    assert!(
        response
            .public_url
            .as_deref()
            .unwrap()
            .starts_with("/media")
    );
    assert!(response.file_size_bytes.unwrap() > 0);
    assert!(response.cost_estimate.is_some());

    let fetched = service
        .get_generated_image(response.id.as_str())
        .await
        .expect("fetch ok")
        .expect("present");
    assert_eq!(fetched.mime_type, "image/png");
    assert_eq!(fetched.user_id.as_ref(), Some(&user_id));
    assert!(fetched.ai_content);
}

#[tokio::test]
async fn generate_image_propagates_provider_error_and_persists_nothing() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::failing("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        Arc::clone(&file_provider),
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let err = service
        .generate_image(request(&user_id, None))
        .await
        .expect_err("provider error");
    assert!(format!("{err}").contains("stub image failure"));
    assert!(file_provider.files.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn generate_image_without_model_or_default_errors() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        file_provider,
        vec![("stub".to_owned(), provider)],
        None,
    );

    let err = service
        .generate_image(request(&user_id, None))
        .await
        .expect_err("no provider");
    assert!(format!("{err}").contains("No model specified"));
}

#[tokio::test]
async fn generate_image_unknown_model_errors() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        file_provider,
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let err = service
        .generate_image(request(&user_id, Some("nonexistent-model")))
        .await
        .expect_err("unknown model");
    assert!(format!("{err}").contains("No provider found for model"));
}

#[tokio::test]
async fn generate_image_routes_by_model_name() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        file_provider,
        vec![("stub".to_owned(), provider)],
        None,
    );

    let response = service
        .generate_image(request(&user_id, Some("stub-image-1")))
        .await
        .expect("routed by model");
    assert_eq!(response.model, "stub-image-1");
}

#[tokio::test]
async fn list_and_delete_user_images_round_trip() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        Arc::clone(&file_provider),
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let first = service
        .generate_image(request(&user_id, None))
        .await
        .expect("first");
    let _second = service
        .generate_image(request(&user_id, None))
        .await
        .expect("second");

    let listed = service
        .list_user_images(&user_id, Some(10), Some(0))
        .await
        .expect("list");
    assert_eq!(listed.len(), 2);

    service
        .delete_image(first.id.as_str())
        .await
        .expect("delete");

    let after = service
        .list_user_images(&user_id, None, None)
        .await
        .expect("list after delete");
    assert_eq!(after.len(), 1);
    assert!(
        service
            .get_generated_image(first.id.as_str())
            .await
            .expect("fetch deleted")
            .is_none()
    );
}

#[tokio::test]
async fn generate_batch_returns_all_responses() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        file_provider,
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let responses = service
        .generate_batch(vec![request(&user_id, None), request(&user_id, None)])
        .await
        .expect("batch");
    assert_eq!(responses.len(), 2);
}

#[tokio::test]
async fn generate_batch_stops_on_first_error() {
    let Some(pool) = pool().await else {
        return;
    };
    let user_id = seed_user(&pool).await;
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::failing("stub", "stub-image-1"));
    let (_dir, service) = build_service(
        &pool,
        file_provider,
        vec![("stub".to_owned(), provider)],
        Some("stub".to_owned()),
    );

    let err = service
        .generate_batch(vec![request(&user_id, None)])
        .await
        .expect_err("batch error");
    assert!(format!("{err}").contains("stub image failure"));
}

#[tokio::test]
async fn provider_registry_accessors_report_state() {
    let Some(pool) = pool().await else {
        return;
    };
    let file_provider = Arc::new(InMemoryFileProvider::default());
    let provider: BoxedImageProvider = Arc::new(StubImageProvider::ok("stub", "stub-image-1"));
    let (_dir, config) = storage_config();
    let mut service = ImageService::new(&pool, config, file_provider).expect("new");

    assert!(service.list_providers().is_empty());
    assert!(service.default_provider_name().is_none());
    assert!(service.get_default_provider().is_none());
    assert!(service.default_provider_capabilities().is_none());

    service.register_provider(provider);
    service.set_default_provider("stub".to_owned());

    assert_eq!(service.list_providers(), vec!["stub".to_owned()]);
    assert_eq!(service.default_provider_name(), Some("stub"));
    assert!(service.get_provider("stub").is_some());
    assert!(service.get_provider("missing").is_none());
    assert!(service.get_default_provider().is_some());
    let caps = service
        .default_provider_capabilities()
        .expect("caps present");
    assert!((caps.cost_per_image_cents - 4.0).abs() < f32::EPSILON);

    let debug = format!("{service:?}");
    assert!(debug.contains("ImageService"));
}
