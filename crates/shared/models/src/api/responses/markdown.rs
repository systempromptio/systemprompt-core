//! Markdown response with structured YAML frontmatter for rendering
//! authored content over the API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;
#[cfg(feature = "web")]
use http::header;

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

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    #[must_use]
    pub fn with_published_at(mut self, published_at: impl Into<String>) -> Self {
        self.published_at = Some(published_at.into());
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
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

    pub fn to_markdown(&self) -> Result<String, serde_yaml::Error> {
        let yaml = self.frontmatter.to_yaml()?;
        Ok(format!("---\n{}---\n\n{}", yaml, self.body))
    }
}

#[cfg(feature = "web")]
impl IntoResponse for MarkdownResponse {
    fn into_response(self) -> axum::response::Response {
        match self.to_markdown() {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                body,
            )
                .into_response(),
            Err(e) => {
                tracing::error!(error = %e, "MarkdownResponse frontmatter serialization failed");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            },
        }
    }
}
