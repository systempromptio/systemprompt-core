//! Error types for the static-site generator pipeline.
//!
//! [`PublishError`] is the unified error type returned by every public function
//! in `systemprompt-generator`. It composes upstream I/O, YAML, and JSON errors
//! via [`From`] so call sites can use `?` without manual mapping, and exposes
//! domain-specific variants (`MissingField`, `TemplateNotFound`,
//! `RenderFailed`, etc.) so CLI/API layers can surface actionable diagnostics.
//!
//! [`GeneratorResult`] is the canonical `Result` alias — prefer it over bare
//! `Result<T, PublishError>` in new code.

use std::path::PathBuf;

mod suggestions;
use suggestions::suggest_fix_for_field;

/// Errors raised by the static-site generator (prerender, sitemap, RSS, asset
/// organisation, build orchestration).
///
/// Variants fall into three groups:
///
/// 1. **Domain errors** (`MissingField`, `TemplateNotFound`, `ProviderFailed`,
///    `RenderFailed`, `FetchFailed`, `Config`, `PagePrerendererFailed`) carry
///    actionable context (slug, template name, suggestions) for surfacing in
///    the CLI/API layer.
/// 2. **Upstream errors** (`Io`, `Yaml`, `Json`) are auto-converted via
///    `#[from]` so `?` works against the host crate's I/O and parsing layers.
/// 3. **`Other`** is a stringly catch-all for upstream errors that do not have
///    a `#[from]` impl. Prefer typed variants when possible.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// Required frontmatter or config field is missing for the given content
    /// item.
    #[error("Missing field '{field}' for content '{slug}'")]
    MissingField {
        /// Name of the missing field (e.g. `title`, `slug`, `image`).
        field: String,
        /// Slug of the offending content item.
        slug: String,
        /// Optional source-file path for diagnostics.
        source_path: Option<PathBuf>,
        /// Optional human-readable fix suggestion.
        suggestion: Option<String>,
    },

    /// No template is registered for the requested content type.
    #[error("No template for content type '{content_type}'")]
    TemplateNotFound {
        /// Content type that was requested (e.g. `blog_post`).
        content_type: String,
        /// Slug of the content item that triggered the lookup.
        slug: String,
        /// Templates currently available — used to suggest a fix.
        available_templates: Vec<String>,
    },

    /// A template-data, page-data, or RSS provider returned an error.
    #[error("Page data provider '{provider_id}' failed: {cause}")]
    ProviderFailed {
        /// Provider identifier as reported by the provider trait.
        provider_id: String,
        /// Display string of the underlying provider error.
        cause: String,
        /// Optional fix suggestion.
        suggestion: Option<String>,
    },

    /// A Handlebars/template render call failed.
    #[error("Template render failed for '{template_name}'")]
    RenderFailed {
        /// Name of the template that failed to render.
        template_name: String,
        /// Slug of the content item being rendered, when known.
        slug: Option<String>,
        /// Display string of the underlying renderer error.
        cause: String,
    },

    /// Fetching content for a configured source failed.
    #[error("Content fetch failed for source '{source_name}'")]
    FetchFailed {
        /// Configured source name (e.g. `blog`, `docs`).
        source_name: String,
        /// Display string of the underlying fetch error.
        cause: String,
    },

    /// A configuration file (`content.yaml`, `web.yaml`, …) is missing,
    /// malformed, or invalid.
    #[error("Configuration error: {message}")]
    Config {
        /// Human-readable description of the configuration problem.
        message: String,
        /// Optional path to the offending config file.
        path: Option<String>,
    },

    /// A registered page-prerenderer extension failed.
    #[error("Page prerenderer '{page_type}' failed: {cause}")]
    PagePrerendererFailed {
        /// Page type identifier reported by the prerenderer.
        page_type: String,
        /// Display string of the underlying prerenderer error.
        cause: String,
    },

    /// Filesystem or network I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parse or serialisation failure.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON parse or serialisation failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for upstream errors without a dedicated `#[from]` variant.
    #[error("{0}")]
    Other(String),
}

/// Canonical `Result` alias for the generator crate.
pub type GeneratorResult<T> = Result<T, PublishError>;

impl PublishError {
    /// Build a [`PublishError::MissingField`] without a source path, with a
    /// suggestion populated from the field name when possible.
    pub fn missing_field(field: impl Into<String>, slug: impl Into<String>) -> Self {
        let field_str = field.into();
        Self::MissingField {
            suggestion: suggest_fix_for_field(&field_str),
            field: field_str,
            slug: slug.into(),
            source_path: None,
        }
    }

    /// Build a [`PublishError::MissingField`] with an attached source-file
    /// path for diagnostics.
    pub fn missing_field_with_path(
        field: impl Into<String>,
        slug: impl Into<String>,
        path: PathBuf,
    ) -> Self {
        let field_str = field.into();
        Self::MissingField {
            suggestion: suggest_fix_for_field(&field_str),
            field: field_str,
            slug: slug.into(),
            source_path: Some(path),
        }
    }

    /// Build a [`PublishError::TemplateNotFound`] carrying the list of
    /// available templates so the CLI layer can suggest alternatives.
    pub fn template_not_found(
        content_type: impl Into<String>,
        slug: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::TemplateNotFound {
            content_type: content_type.into(),
            slug: slug.into(),
            available_templates: available,
        }
    }

    /// Build a [`PublishError::ProviderFailed`] without a fix suggestion.
    pub fn provider_failed(provider_id: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::ProviderFailed {
            provider_id: provider_id.into(),
            cause: cause.into(),
            suggestion: None,
        }
    }

    /// Build a [`PublishError::RenderFailed`] with the optional slug of the
    /// content item being rendered.
    pub fn render_failed(
        template_name: impl Into<String>,
        slug: Option<String>,
        cause: impl Into<String>,
    ) -> Self {
        Self::RenderFailed {
            template_name: template_name.into(),
            slug,
            cause: cause.into(),
        }
    }

    /// Build a [`PublishError::FetchFailed`].
    pub fn fetch_failed(source_name: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::FetchFailed {
            source_name: source_name.into(),
            cause: cause.into(),
        }
    }

    /// Build a [`PublishError::Config`] without a file path.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            path: None,
        }
    }

    /// Build a [`PublishError::PagePrerendererFailed`].
    pub fn page_prerenderer_failed(page_type: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::PagePrerendererFailed {
            page_type: page_type.into(),
            cause: cause.into(),
        }
    }

    /// Build a [`PublishError::Other`] from any `Display` value. Used for
    /// upstream errors without a typed `#[from]` impl.
    pub fn other(cause: impl std::fmt::Display) -> Self {
        Self::Other(cause.to_string())
    }

    /// Filesystem path associated with this error, if any. Used by CLI
    /// formatters to show "in file: …" hints.
    pub fn location(&self) -> Option<String> {
        match self {
            Self::MissingField { source_path, .. } => {
                source_path.as_ref().map(|p| p.display().to_string())
            },
            Self::Config { path, .. } => path.clone(),
            _ => None,
        }
    }

    /// Human-readable fix suggestion, if the variant carries one.
    pub fn suggestion_string(&self) -> Option<String> {
        match self {
            Self::MissingField { suggestion, .. } | Self::ProviderFailed { suggestion, .. } => {
                suggestion.clone()
            },
            Self::TemplateNotFound {
                available_templates,
                content_type,
                ..
            } => {
                if available_templates.is_empty() {
                    Some("Add templates to the templates directory".to_string())
                } else {
                    Some(format!(
                        "Change content type from '{}' to one of: {}",
                        content_type,
                        available_templates.join(", ")
                    ))
                }
            },
            _ => None,
        }
    }

    /// Underlying upstream-error display string, when the variant exposes one.
    pub fn cause_string(&self) -> Option<String> {
        match self {
            Self::ProviderFailed { cause, .. }
            | Self::RenderFailed { cause, .. }
            | Self::FetchFailed { cause, .. }
            | Self::PagePrerendererFailed { cause, .. } => Some(cause.clone()),
            _ => None,
        }
    }
}
