//! Markdown response with structured YAML frontmatter for rendering
//! authored content over the API.

use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;
#[cfg(feature = "web")]
use http::header;

/// Structured frontmatter block prepended to a markdown response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownFrontmatter {
    /// Document title.
    pub title: String,
    /// URL slug.
    pub slug: String,
    /// Optional summary description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional author byline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional ISO publication timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Optional list of tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional canonical URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl MarkdownFrontmatter {
    /// Build a frontmatter block carrying just `title` and `slug`.
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

    /// Attach a description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Attach an author byline.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Attach a publication timestamp.
    #[must_use]
    pub fn with_published_at(mut self, published_at: impl Into<String>) -> Self {
        self.published_at = Some(published_at.into());
        self
    }

    /// Replace the tag list.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Attach a canonical URL.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Render this frontmatter block as a YAML string suitable for
    /// embedding between `---` delimiters.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`serde_yaml::Error`] when serialization fails.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
}

/// Markdown response combining a [`MarkdownFrontmatter`] block with a
/// raw body string.
#[derive(Debug, Clone)]
pub struct MarkdownResponse {
    /// Structured frontmatter rendered as YAML in the wire payload.
    pub frontmatter: MarkdownFrontmatter,
    /// Raw markdown body.
    pub body: String,
}

impl MarkdownResponse {
    /// Pair `frontmatter` with `body`.
    pub fn new(frontmatter: MarkdownFrontmatter, body: impl Into<String>) -> Self {
        Self {
            frontmatter,
            body: body.into(),
        }
    }

    /// Render the response as a markdown document with YAML frontmatter
    /// fenced by `---` delimiters.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`serde_yaml::Error`] when frontmatter
    /// serialization fails.
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
