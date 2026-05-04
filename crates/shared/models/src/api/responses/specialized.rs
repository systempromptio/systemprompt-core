//! Specialized response envelopes for `201 Created`, `202 Accepted`,
//! plain success messages, and HATEOAS-style discovery payloads.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::envelopes::ResponseMeta;

#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;

/// Plain success response with a human-readable message.
#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse {
    /// Free-form success message.
    pub message: String,

    /// Envelope metadata.
    pub meta: ResponseMeta,
}

impl SuccessResponse {
    /// Build a [`SuccessResponse`] with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            meta: ResponseMeta::new(),
        }
    }
}

/// `201 Created` response carrying the new entity and its `Location` header.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedResponse<T>
where
    T: 'static,
{
    /// The newly-created entity.
    pub data: T,

    /// Envelope metadata.
    pub meta: ResponseMeta,

    /// URL of the new resource (mirrored in the HTTP `Location` header).
    pub location: String,
}

impl<T: Serialize + 'static> CreatedResponse<T> {
    /// Build a [`CreatedResponse`] for `data` exposed at `location`.
    pub fn new(data: T, location: impl Into<String>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            location: location.into(),
        }
    }
}

/// `202 Accepted` response indicating a job has been queued.
#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptedResponse {
    /// Free-form acknowledgement message.
    pub message: String,

    /// Optional id of the queued job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    /// Optional URL clients can poll to check job status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_url: Option<String>,

    /// Envelope metadata.
    pub meta: ResponseMeta,
}

impl AcceptedResponse {
    /// Build an [`AcceptedResponse`] with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            job_id: None,
            status_url: None,
            meta: ResponseMeta::new(),
        }
    }

    /// Attach the queued job id and status URL.
    #[must_use]
    pub fn with_job(mut self, job_id: impl Into<String>, status_url: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self.status_url = Some(status_url.into());
        self
    }
}

/// Single hypermedia link entry in a [`DiscoveryResponse`].
#[derive(Debug, Serialize, Deserialize)]
pub struct Link {
    /// URL the link points at.
    pub href: String,
    /// Optional human-readable title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Link {
    /// Build a new [`Link`] with the given href and optional title.
    pub fn new(href: impl Into<String>, title: Option<String>) -> Self {
        Self {
            href: href.into(),
            title,
        }
    }
}

/// Discovery / index response embedding a payload alongside a named
/// link map.
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryResponse<T>
where
    T: 'static,
{
    /// Response payload.
    pub data: T,
    /// Envelope metadata.
    pub meta: ResponseMeta,
    /// Named hypermedia links keyed by relation name.
    #[serde(rename = "_links")]
    pub links: IndexMap<String, Link>,
}

impl<T: Serialize + 'static> DiscoveryResponse<T> {
    /// Build a [`DiscoveryResponse`] for `data` and the supplied link map.
    pub fn new(data: T, links: IndexMap<String, Link>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links,
        }
    }

    /// Override the envelope metadata.
    #[must_use]
    pub fn with_meta(mut self, meta: ResponseMeta) -> Self {
        self.meta = meta;
        self
    }
}

#[cfg(feature = "web")]
impl IntoResponse for SuccessResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[cfg(feature = "web")]
impl<T: Serialize + 'static> IntoResponse for CreatedResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::CREATED,
            [("Location", self.location.clone())],
            Json(self),
        )
            .into_response()
    }
}

#[cfg(feature = "web")]
impl IntoResponse for AcceptedResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::ACCEPTED, Json(self)).into_response()
    }
}
