//! Document creation, metadata, rename/move and trash.
//!
//! `POST /` takes JSON to create from a template, or `multipart/form-data`
//! to import an uploaded file (parts `file`, optional `name`, `folder_id`,
//! `format`). A multipart file is first stored as an upload, so it passes
//! the same content checks, quota and recorded MIME type as the resumable
//! API, and is then imported from that blob exactly as `POST /import` does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{FromRequest, Multipart, Path, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Deserializer};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::ListCursor;
use systemprompt_files::documents::service::{
    CreateRequest, FileCreateRequest, FolderMove, UpdateRequest,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentTemplateId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_provider_contracts::documents::TemplateParams;

use super::DocumentsState;
use super::caller::{api_author, caller};
use super::error::DocumentsResult;
use super::headers::set_revision_tags;
use super::items_multipart::read_file;
use super::requests::with_request_hint;
use super::views::{CreatedJson, DocumentDetail};

pub(super) fn document_id(raw: &str) -> DocumentsResult<DocumentId> {
    Ok(DocumentId::try_new(raw)?)
}

pub(super) fn folder_id(raw: Option<&str>) -> DocumentsResult<Option<DocumentFolderId>> {
    Ok(raw
        .filter(|raw| !raw.is_empty())
        .map(DocumentFolderId::try_new)
        .transpose()?)
}

pub(super) fn cursor(raw: Option<&str>) -> DocumentsResult<Option<ListCursor>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    match raw.parse::<ListCursor>() {
        Ok(cursor) => Ok(Some(cursor)),
        Err(source) => Err(ApiError::bad_request("cursor is not valid")
            .with_source(source)
            .into()),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum FolderTarget {
    #[default]
    Keep,
    Root,
    Folder(DocumentFolderId),
}

impl FolderTarget {
    pub(super) const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }

    pub(super) fn into_move(self) -> FolderMove {
        match self {
            Self::Keep => FolderMove::Keep,
            Self::Root => FolderMove::Root,
            Self::Folder(id) => FolderMove::Into(id),
        }
    }
}

pub(super) fn folder_target<'de, D>(deserializer: D) -> Result<FolderTarget, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<DocumentFolderId>::deserialize(deserializer)?
        .map_or(FolderTarget::Root, FolderTarget::Folder))
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateBody {
    template_id: DocumentTemplateId,
    #[serde(default)]
    folder_id: Option<DocumentFolderId>,
    name: String,
    #[serde(default)]
    params: TemplateParams,
}

pub(super) async fn create(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    request: Request,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let author = api_author(&req_ctx, request.headers());
    let multipart = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("multipart/form-data"));
    let (document, revision) = if multipart {
        let form = Multipart::from_request(request, &state).await?;
        let file = read_file(form).await?;
        let create = FileCreateRequest {
            name: file.name,
            folder: file.folder,
            format: file.format,
            mime: file.mime,
            bytes: file.bytes,
            author,
        };
        state
            .service
            .create_from_file(&who.user, create, &who.trace)
            .await?
    } else {
        let Json(body) = Json::<CreateBody>::from_request(request, &state).await?;
        let create = CreateRequest {
            template_id: body.template_id,
            folder: body.folder_id,
            name: body.name,
            params: body.params,
            ops: Vec::new(),
            author,
            message: None,
            content: None,
            media: Media::new(),
        };
        state
            .service
            .create_from_template(&who.user, create, &who.trace)
            .await?
    };
    let created = CreatedJson {
        document: document.into(),
        revision: revision.into(),
    };
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

pub(super) async fn get(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    method: Method,
    Path(id): Path<String>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let view = match state.service.get(&who.user, &id, &who.trace).await {
        Ok(view) => view,
        Err(error) => {
            return with_request_hint::<Response>(&state, &who, &id, Err(error.into())).await;
        },
    };
    let head = view
        .head
        .as_ref()
        .map(|head| (head.id.clone(), head.content_sha256.clone()));
    let mut response = if method == Method::HEAD {
        StatusCode::OK.into_response()
    } else {
        Json(DocumentDetail::from(view)).into_response()
    };
    if let Some((revision, hash)) = head {
        set_revision_tags(response.headers_mut(), &revision, &hash);
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "folder_target")]
    folder_id: FolderTarget,
}

pub(super) async fn update(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    if body.name.is_none() && body.folder_id.is_keep() {
        return Err(ApiError::bad_request("give a new `name`, a `folder_id`, or both").into());
    }
    let folder = body.folder_id.into_move();
    let document = state
        .service
        .update(
            &who.user,
            &id,
            UpdateRequest {
                name: body.name,
                folder,
            },
            &who.trace,
        )
        .await?;
    Ok(Json(super::views::DocumentJson::from(document)).into_response())
}

pub(super) async fn delete(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    state.service.trash(&who.user, &id, &who.trace).await?;
    Ok(StatusCode::NO_CONTENT)
}
