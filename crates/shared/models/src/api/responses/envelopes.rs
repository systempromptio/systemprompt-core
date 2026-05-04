//! Primary HATEOAS-style response envelopes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::pagination::PaginationInfo;

#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;

/// Hypermedia-style link block attached to a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseLinks {
    /// Canonical URL of the resource being returned.
    pub self_link: String,

    /// URL of the next page when this is a paginated collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,

    /// URL of the previous page when this is a paginated collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,

    /// URL of the documentation for this resource.
    pub docs: String,
}

/// Envelope metadata: timestamp, API version, optional pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    /// Server-side timestamp when the response was assembled.
    pub timestamp: DateTime<Utc>,

    /// API version string.
    pub version: String,

    /// Pagination block when the response is a paged collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationInfo>,
}

impl ResponseMeta {
    /// Build a new [`ResponseMeta`] stamped with the current time.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            pagination: None,
        }
    }

    /// Attach a [`PaginationInfo`] block to this metadata.
    #[must_use]
    pub fn with_pagination(mut self, pagination: PaginationInfo) -> Self {
        self.pagination = Some(pagination);
        self
    }
}

impl Default for ResponseMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic response envelope wrapping an arbitrary serializable payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T>
where
    T: 'static,
{
    /// Response payload.
    pub data: T,

    /// Envelope metadata.
    pub meta: ResponseMeta,

    /// Optional hypermedia links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> ApiResponse<T> {
    /// Wrap `data` in an envelope with fresh metadata and no links.
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    /// Attach a [`ResponseLinks`] block.
    #[must_use]
    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }

    /// Override the envelope metadata.
    #[must_use]
    pub fn with_meta(mut self, meta: ResponseMeta) -> Self {
        self.meta = meta;
        self
    }
}

/// Single-entity response envelope used by `GET /resources/:id` style
/// endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct SingleResponse<T>
where
    T: 'static,
{
    /// Response payload.
    pub data: T,

    /// Envelope metadata.
    pub meta: ResponseMeta,

    /// Optional hypermedia links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> SingleResponse<T> {
    /// Wrap `data` in a single-entity envelope.
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    /// Construct directly with a pre-built [`ResponseMeta`] (const-friendly).
    pub const fn with_meta(data: T, meta: ResponseMeta) -> Self {
        Self {
            data,
            meta,
            links: None,
        }
    }

    /// Attach a [`ResponseLinks`] block.
    #[must_use]
    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }
}

/// Collection response envelope used by `GET /resources` style endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionResponse<T>
where
    T: 'static,
{
    /// Items in the collection.
    pub data: Vec<T>,

    /// Envelope metadata.
    pub meta: ResponseMeta,

    /// Optional hypermedia links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> CollectionResponse<T> {
    /// Wrap `data` in a collection envelope.
    pub fn new(data: Vec<T>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    /// Wrap `data` in a collection envelope with attached pagination.
    pub fn paginated(data: Vec<T>, pagination: PaginationInfo) -> Self {
        Self {
            data,
            meta: ResponseMeta::new().with_pagination(pagination),
            links: None,
        }
    }

    /// Attach a [`ResponseLinks`] block.
    #[must_use]
    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }
}

#[cfg(feature = "web")]
impl<T: Serialize + 'static> IntoResponse for SingleResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[cfg(feature = "web")]
impl<T: Serialize + 'static> IntoResponse for CollectionResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}
