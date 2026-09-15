//! Persists image-generation requests and produced file records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, Result};
use crate::models::AiRequestRecordBuilder;
use crate::models::image_generation::{ImageGenerationRequest, ImageGenerationResponse};
use crate::repository::AiRequestRepository;
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_traits::{
    AiFilePersistenceProvider, AiGeneratedFile, ImageGenerationInfo, ImageMetadata,
    InsertAiFileParams,
};
use uuid::Uuid;

pub(super) struct FileLocation<'a> {
    pub path: &'a str,
    pub public_url: &'a str,
}

pub(super) async fn persist_image_generation(
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
    let context_id = request.session_id.as_ref().map_or_else(
        systemprompt_identifiers::ContextId::legacy,
        systemprompt_identifiers::ContextId::derived_from_session,
    );
    let mut builder = AiRequestRecordBuilder::new(
        response.request_id.clone(),
        request.user_id.clone(),
        context_id,
    )
    .provider(&response.provider)
    .model(&response.model)
    .cost(response.cost_estimate.map_or(0, cents_to_microdollars))
    .latency(response.generation_time_ms as i32)
    .completed();

    if let Some(session_id) = &request.session_id {
        builder = builder.session_id(session_id.clone());
    }

    if let Some(trace_id) = &request.trace_id {
        builder = builder.trace_id(trace_id.clone());
    }

    if let Some(mcp_execution_id) = &request.mcp_execution_id {
        builder = builder.mcp_execution_id(mcp_execution_id.clone());
    }

    let record = builder.build();

    ai_request_repo
        .insert(&record)
        .await
        .map(|_| ())
        .map_err(|e| AiError::DatabaseError {
            message: e.to_string(),
        })
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

    let file_id = Uuid::parse_str(response.id.as_str())
        .map(|uuid| FileId::new(uuid.to_string()))
        .map_err(|e| AiError::InvalidInput(format!("Invalid UUID: {e}")))?;

    let params =
        InsertAiFileParams::new(file_id, file_path, public_url, response.mime_type.clone())
            .with_size_bytes(response.file_size_bytes.map(|s| s as i64))
            .with_metadata(metadata)
            .with_user_id(Some(request.user_id.clone()))
            .with_session_id(request.session_id.clone())
            .with_trace_id(request.trace_id.clone());

    file_provider
        .insert_file(params)
        .await
        .map_err(|e| AiError::DatabaseError {
            message: e.to_string(),
        })
}

pub(super) async fn find_generated_image(
    file_provider: &dyn AiFilePersistenceProvider,
    uuid: &str,
) -> Result<Option<AiGeneratedFile>> {
    file_provider
        .find_by_id(&FileId::new(uuid))
        .await
        .map_err(|e| AiError::DatabaseError {
            message: e.to_string(),
        })
}

pub(super) async fn list_user_images(
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
        .map_err(|e| AiError::DatabaseError {
            message: e.to_string(),
        })
}

pub(super) async fn delete_image(
    file_provider: &dyn AiFilePersistenceProvider,
    storage: &crate::services::storage::ImageStorage,
    uuid: &str,
) -> Result<()> {
    let file_id = FileId::new(uuid);
    let file = file_provider
        .find_by_id(&file_id)
        .await
        .map_err(|e| AiError::DatabaseError {
            message: e.to_string(),
        })?;

    if let Some(file_record) = file {
        storage
            .delete_image(&systemprompt_traits::StoredFileId::new(
                file_record.path.clone(),
            ))
            .await?;
        file_provider
            .delete(&file_id)
            .await
            .map_err(|e| AiError::DatabaseError {
                message: e.to_string(),
            })?;
    }

    Ok(())
}

// Why: `cost_estimate` is priced in cents while `ai_requests.cost_microdollars`
// is billed in microdollars; one cent is ten thousand microdollars.
fn cents_to_microdollars(cents: f32) -> i64 {
    (f64::from(cents) * 10_000.0).round() as i64
}
