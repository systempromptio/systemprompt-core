use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageGenerationResponse};
use crate::models::AiRequestRecordBuilder;
use crate::repository::AiRequestRepository;
use systemprompt_identifiers::{FileId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_traits::{
    AiFilePersistenceProvider, AiGeneratedFile, ImageGenerationInfo, ImageMetadata,
    InsertAiFileParams,
};
use uuid::Uuid;

pub struct FileLocation<'a> {
    pub path: &'a str,
    pub public_url: &'a str,
}

pub async fn persist_image_generation(
    ai_request_repo: &AiRequestRepository,
    file_provider: &dyn AiFilePersistenceProvider,
    request: &ImageGenerationRequest,
    response: &ImageGenerationResponse,
    location: FileLocation<'_>,
) -> Result<()> {
    persist_ai_request(ai_request_repo, request, response).await?;
    persist_file_record(
        file_provider,
        request,
        response,
        location.path,
        location.public_url,
    )
    .await
}

async fn persist_ai_request(
    ai_request_repo: &AiRequestRepository,
    request: &ImageGenerationRequest,
    response: &ImageGenerationResponse,
) -> Result<()> {
    let user_id = UserId::new(request.user_id.as_deref().unwrap_or("anonymous"));

    let mut builder = AiRequestRecordBuilder::new(&response.request_id, user_id)
        .provider(&response.provider)
        .model(&response.model)
        .cost(response.cost_estimate.map_or(0, |c| c.round() as i64))
        .latency(response.generation_time_ms as i32)
        .completed();

    if let Some(session_id) = &request.session_id {
        builder = builder.session_id(SessionId::new(session_id));
    }

    if let Some(trace_id) = &request.trace_id {
        builder = builder.trace_id(TraceId::new(trace_id));
    }

    if let Some(mcp_execution_id) = &request.mcp_execution_id {
        builder = builder.mcp_execution_id(McpExecutionId::new(mcp_execution_id));
    }

    let record = builder
        .build()
        .map_err(|e| AiError::InvalidInput(e.to_string()))?;

    ai_request_repo
        .insert(&record)
        .await
        .map(|_| ())
        .map_err(|e| AiError::DatabaseError(e.into()))
}

async fn persist_file_record(
    file_provider: &dyn AiFilePersistenceProvider,
    request: &ImageGenerationRequest,
    response: &ImageGenerationResponse,
    file_path: &str,
    public_url: &str,
) -> Result<()> {
    let generation_info =
        ImageGenerationInfo::new(&request.prompt, &response.model, &response.provider)
            .with_resolution(response.resolution.as_str())
            .with_aspect_ratio(response.aspect_ratio.as_str())
            .with_generation_time(response.generation_time_ms as i32)
            .with_request_id(&response.request_id);

    let generation_info = match response.cost_estimate {
        Some(cost) => generation_info.with_cost_estimate(cost),
        None => generation_info,
    };

    let image_metadata = ImageMetadata::new().with_generation(generation_info);
    let metadata = serde_json::to_value(image_metadata).map_err(AiError::SerializationError)?;

    let id = Uuid::parse_str(&response.id)
        .map_err(|e| AiError::InvalidInput(format!("Invalid UUID: {e}")))?;

    let params = InsertAiFileParams {
        id,
        path: file_path.to_string(),
        public_url: public_url.to_string(),
        mime_type: response.mime_type.clone(),
        size_bytes: response.file_size_bytes.map(|s| s as i64),
        metadata,
        user_id: request.user_id.as_ref().map(UserId::new),
        session_id: request.session_id.as_ref().map(SessionId::new),
        trace_id: request.trace_id.as_ref().map(TraceId::new),
        context_id: None,
    };

    file_provider
        .insert_file(params)
        .await
        .map_err(|e| AiError::DatabaseError(anyhow::anyhow!("{}", e)))
}

pub async fn get_generated_image(
    file_provider: &dyn AiFilePersistenceProvider,
    uuid: &str,
) -> Result<Option<AiGeneratedFile>> {
    file_provider
        .find_by_id(&FileId::new(uuid))
        .await
        .map_err(|e| AiError::DatabaseError(anyhow::anyhow!("{}", e)))
}

pub async fn list_user_images(
    file_provider: &dyn AiFilePersistenceProvider,
    user_id: &UserId,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AiGeneratedFile>> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    file_provider
        .list_by_user(user_id, limit, offset)
        .await
        .map_err(|e| AiError::DatabaseError(anyhow::anyhow!("{}", e)))
}

pub async fn delete_image(
    file_provider: &dyn AiFilePersistenceProvider,
    storage: &crate::services::storage::ImageStorage,
    uuid: &str,
) -> Result<()> {
    let file_id = FileId::new(uuid);
    let file = file_provider
        .find_by_id(&file_id)
        .await
        .map_err(|e| AiError::DatabaseError(anyhow::anyhow!("{}", e)))?;

    if let Some(file_record) = file {
        let file_path = std::path::Path::new(&file_record.path);
        storage.delete_image(file_path)?;
        file_provider
            .delete(&file_id)
            .await
            .map_err(|e| AiError::DatabaseError(anyhow::anyhow!("{}", e)))?;
    }

    Ok(())
}
