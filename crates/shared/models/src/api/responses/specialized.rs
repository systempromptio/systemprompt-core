//! Specialized response envelopes for `201 Created`, `202 Accepted`,
//! plain success messages, and HATEOAS-style discovery payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::envelopes::ResponseMeta;

#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub message: String,

    pub meta: ResponseMeta,
}

impl SuccessResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            meta: ResponseMeta::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedResponse<T>
where
    T: 'static,
{
    pub data: T,

    pub meta: ResponseMeta,

    pub location: String,
}

impl<T: Serialize + 'static> CreatedResponse<T> {
    pub fn new(data: T, location: impl Into<String>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            location: location.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptedResponse {
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_url: Option<String>,

    pub meta: ResponseMeta,
}

impl AcceptedResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            job_id: None,
            status_url: None,
            meta: ResponseMeta::new(),
        }
    }

    #[must_use]
    pub fn with_job(mut self, job_id: impl Into<String>, status_url: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self.status_url = Some(status_url.into());
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Link {
    pub href: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Link {
    pub fn new(href: impl Into<String>, title: Option<String>) -> Self {
        Self {
            href: href.into(),
            title,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryResponse<T>
where
    T: 'static,
{
    pub data: T,
    pub meta: ResponseMeta,
    #[serde(rename = "_links")]
    pub links: IndexMap<String, Link>,
}

impl<T: Serialize + 'static> DiscoveryResponse<T> {
    pub fn new(data: T, links: IndexMap<String, Link>) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
            links,
        }
    }

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
