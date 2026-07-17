//! Primary HATEOAS-style response envelopes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::pagination::PaginationInfo;

#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseLinks {
    pub self_link: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,

    pub docs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub timestamp: DateTime<Utc>,

    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationInfo>,
}

impl ResponseMeta {
    #[must_use]
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            version: "1.0.0".to_owned(),
            pagination: None,
        }
    }

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T>
where
    T: 'static,
{
    pub data: T,

    pub meta: ResponseMeta,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    #[must_use]
    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }

    #[must_use]
    pub fn with_meta(mut self, meta: ResponseMeta) -> Self {
        self.meta = meta;
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SingleResponse<T>
where
    T: 'static,
{
    pub data: T,

    pub meta: ResponseMeta,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> SingleResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    pub const fn with_meta(data: T, meta: ResponseMeta) -> Self {
        Self {
            data,
            meta,
            links: None,
        }
    }

    #[must_use]
    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionResponse<T>
where
    T: 'static,
{
    pub data: Vec<T>,

    pub meta: ResponseMeta,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ResponseLinks>,
}

impl<T: Serialize + 'static> CollectionResponse<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links: None,
        }
    }

    pub fn paginated(data: Vec<T>, pagination: PaginationInfo) -> Self {
        Self {
            data,
            meta: ResponseMeta::new().with_pagination(pagination),
            links: None,
        }
    }

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
