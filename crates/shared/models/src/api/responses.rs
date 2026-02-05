use super::pagination::PaginationInfo;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;
#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use http::header;

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
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            pagination: None,
        }
    }

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

    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }

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

    pub fn with_links(mut self, links: ResponseLinks) -> Self {
        self.links = Some(links);
        self
    }
}

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

    pub job_id: Option<String>,

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

    pub fn with_meta(mut self, meta: ResponseMeta) -> Self {
        self.meta = meta;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownFrontmatter {
    pub title: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl MarkdownFrontmatter {
    pub fn new(title: impl Into<String>, slug: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            slug: slug.into(),
            description: None,
            author: None,
            published_at: None,
            tags: Vec::new(),
            url: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn with_published_at(mut self, published_at: impl Into<String>) -> Self {
        self.published_at = Some(published_at.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct MarkdownResponse {
    pub frontmatter: MarkdownFrontmatter,
    pub body: String,
}

impl MarkdownResponse {
    pub fn new(frontmatter: MarkdownFrontmatter, body: impl Into<String>) -> Self {
        Self {
            frontmatter,
            body: body.into(),
        }
    }

    pub fn to_markdown(&self) -> String {
        format!("---\n{}---\n\n{}", self.frontmatter.to_yaml(), self.body)
    }
}

#[cfg(feature = "web")]
impl IntoResponse for MarkdownResponse {
    fn into_response(self) -> axum::response::Response {
        let body = self.to_markdown();
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            body,
        )
            .into_response()
    }
}
