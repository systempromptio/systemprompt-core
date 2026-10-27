//! Document listing, creation, metadata, rename/move and delete.
//!
//! `POST /` takes JSON to create from a template, or `multipart/form-data`
//! to import an uploaded file (parts `file`, optional `name`, `folder_id`,
//! `format`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{FromRequest, Multipart, Path, Query, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Deserializer};
use systemprompt_files::documents::models::ListCursor;
use systemprompt_files::documents::service::{
    Author, CreateRequest, ImportRequest, ListFolderRequest, RevisionOrigin,
};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentTemplateId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{ActorKind, DocumentFormat};
use systemprompt_provider_contracts::documents::TemplateParams;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::headers::etag;
use super::views::{CreatedJson, DocumentDetail, ListingJson};

const DEFAULT_PAGE: i64 = 50;
const MAX_PAGE: i64 = 200;

pub(super) const fn api_author() -> Author {
    Author {
        actor_kind: ActorKind::User,
        origin: RevisionOrigin::Api,
    }
}

pub(super) fn document_id(raw: &str) -> Result<DocumentId, ApiError> {
    Ok(DocumentId::try_new(raw)?)
}

pub(super) fn folder_id(raw: Option<&str>) -> Result<Option<DocumentFolderId>, ApiError> {
    raw.filter(|raw| !raw.is_empty())
        .map(DocumentFolderId::try_new)
        .transpose()
        .map_err(ApiError::from)
}

pub(super) fn cursor(raw: Option<&str>) -> Result<Option<ListCursor>, ApiError> {
    raw.map(|raw| {
        raw.parse::<ListCursor>()
            .map_err(|source| ApiError::bad_request("cursor is not valid").with_source(source))
    })
    .transpose()
}

pub(super) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
pub(super) struct ListQuery {
    folder: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
}

pub(super) async fn list(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(query): Query<ListQuery>,
) -> DocumentsResult<Json<ListingJson>> {
    let who = caller(&state, &req_ctx).await?;
    let request = ListFolderRequest {
        folder: folder_id(query.folder.as_deref())?,
        cursor: cursor(query.cursor.as_deref())?,
        limit: query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE),
    };
    let listing = state
        .service
        .list_folder(&who.user, request, &who.trace)
        .await?;
    Ok(Json(listing.into()))
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateBody {
    template_id: String,
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
    let multipart = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("multipart/form-data"));
    let (document, revision) = if multipart {
        let form = Multipart::from_request(request, &state)
            .await
            .map_err(|source| ApiError::bad_request("invalid multipart body").with_source(source))?;
        let import = read_import(form).await?;
        state.service.import(&who.user, import, &who.trace).await?
    } else {
        let Json(body) = Json::<CreateBody>::from_request(request, &state)
            .await
            .map_err(|source| ApiError::bad_request("invalid request body").with_source(source))?;
        let create = CreateRequest {
            template_id: DocumentTemplateId::try_new(&body.template_id).map_err(ApiError::from)?,
            folder: body.folder_id,
            name: body.name,
            params: body.params,
            author: api_author(),
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

async fn read_import(mut form: Multipart) -> Result<ImportRequest, ApiError> {
    let mut name = None;
    let mut folder = None;
    let mut format = None;
    let mut file: Option<(Option<String>, Vec<u8>)> = None;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|source| ApiError::bad_request("invalid multipart body").with_source(source))?
    {
        let part = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().map(str::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|source| ApiError::bad_request("invalid multipart part").with_source(source))?;
        let text = || String::from_utf8_lossy(&bytes).trim().to_owned();
        match part.as_str() {
            "file" => file = Some((file_name, bytes.to_vec())),
            "name" => name = Some(text()),
            "folder_id" => folder = folder_id(Some(&text()))?,
            "format" => {
                format = Some(text().parse::<DocumentFormat>().map_err(|source| {
                    ApiError::bad_request("format is not a known document format")
                        .with_source(source)
                })?);
            },
            _ => {},
        }
    }
    let (file_name, bytes) =
        file.ok_or_else(|| ApiError::bad_request("multipart import needs a `file` part"))?;
    let name = name
        .or(file_name)
        .ok_or_else(|| ApiError::bad_request("multipart import needs a `name`"))?;
    Ok(ImportRequest {
        folder,
        name,
        format,
        bytes,
        author: api_author(),
    })
}

pub(super) async fn get(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    method: Method,
    Path(id): Path<String>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let view = state.service.get(&who.user, &id, &who.trace).await?;
    let tag = view.head.as_ref().map(|head| etag(&head.content_sha256));
    let mut response = if method == Method::HEAD {
        StatusCode::OK.into_response()
    } else {
        Json(DocumentDetail::from(view)).into_response()
    };
    if let Some(tag) = tag.and_then(|tag| HeaderValue::from_str(&tag).ok()) {
        response.headers_mut().insert(header::ETAG, tag);
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    folder_id: Option<Option<DocumentFolderId>>,
}

pub(super) async fn update(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    if body.name.is_none() && body.folder_id.is_none() {
        return Err(ApiError::bad_request("give a new `name`, a `folder_id`, or both").into());
    }
    let mut document = None;
    if let Some(folder) = body.folder_id {
        document = Some(
            state
                .service
                .move_to(&who.user, &id, folder.as_ref(), &who.trace)
                .await?,
        );
    }
    if let Some(name) = body.name {
        document = Some(
            state
                .service
                .rename(&who.user, &id, &name, &who.trace)
                .await?,
        );
    }
    let detail = document.map(super::views::DocumentJson::from);
    Ok(Json(detail).into_response())
}

pub(super) async fn delete(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    state.service.delete(&who.user, &id, &who.trace).await?;
    Ok(StatusCode::NO_CONTENT)
}

