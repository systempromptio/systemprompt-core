use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageGenerationResponse};
use crate::models::AiRequestRecordBuilder;
use crate::repository::AiRequestRepository;
use chrono::Utc;
use systemprompt_files::{File, FileMetadata, FileRepository, ImageGenerationInfo, ImageMetadata};
use systemprompt_identifiers::{FileId, McpExecutionId, SessionId, TraceId, UserId};
use uuid::Uuid;

pub struct FileLocation<'a> {
    pub path: &'a str,
    pub public_url: &'a str,
}

pub async fn persist_image_generation(
    ai_request_repo: &AiRequestRepository,
    file_repo: &FileRepository,
    request: &ImageGenerationRequest,
    response: &ImageGenerationResponse,
    location: FileLocation<'_>,
) -> Result<()> {
    persist_ai_request(ai_request_repo, request, response).await?;
    persist_file_record(
        file_repo,
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
        .cost(response.cost_estimate.map_or(0, |c| c.round() as i32))
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
    file_repo: &FileRepository,
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
    let metadata = serde_json::to_value(FileMetadata::new().with_image(image_metadata))
        .map_err(AiError::SerializationError)?;

    let now = Utc::now();
    let file = File {
        id: Uuid::parse_str(&response.id)
            .map_err(|e| AiError::InvalidInput(format!("Invalid UUID: {e}")))?,
        path: file_path.to_string(),
        public_url: public_url.to_string(),
        mime_type: response.mime_type.clone(),
        size_bytes: response.file_size_bytes.map(|s| s as i64),
        ai_content: true,
        metadata,
        user_id: request.user_id.as_ref().map(UserId::new),
        session_id: request.session_id.as_ref().map(SessionId::new),
        trace_id: request.trace_id.as_ref().map(TraceId::new),
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    file_repo.insert_file(&file).await.map(|_| ()).map_err(|e| {
        AiError::DatabaseError(anyhow::anyhow!(
            "Failed to persist generated image (id: {}, path: {}, url: {}): {}",
            response.id,
            file_path,
            public_url,
            e
        ))
    })
}

pub async fn get_generated_image(file_repo: &FileRepository, uuid: &str) -> Result<Option<File>> {
    file_repo
        .find_by_id(&FileId::new(uuid))
        .await
        .map_err(AiError::DatabaseError)
}

pub async fn list_user_images(
    file_repo: &FileRepository,
    user_id: &UserId,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<File>> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    file_repo
        .list_by_user(user_id, limit, offset)
        .await
        .map_err(AiError::DatabaseError)
}

pub async fn delete_image(
    file_repo: &FileRepository,
    storage: &crate::services::storage::ImageStorage,
    uuid: &str,
) -> Result<()> {
    let file_id = FileId::new(uuid);
    let file = file_repo.find_by_id(&file_id).await?;

    if let Some(file_record) = file {
        let file_path = std::path::Path::new(&file_record.path);
        storage.delete_image(file_path)?;
        file_repo.delete(&file_id).await?;
    }

    Ok(())
}
