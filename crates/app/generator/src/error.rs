use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Missing field '{field}' for content '{slug}'")]
    MissingField {
        field: String,
        slug: String,
        source_path: Option<PathBuf>,
        suggestion: Option<String>,
    },

    #[error("No template for content type '{content_type}'")]
    TemplateNotFound {
        content_type: String,
        slug: String,
        available_templates: Vec<String>,
    },

    #[error("Page data provider '{provider_id}' failed: {cause}")]
    ProviderFailed {
        provider_id: String,
        cause: String,
        suggestion: Option<String>,
    },

    #[error("Template render failed for '{template_name}'")]
    RenderFailed {
        template_name: String,
        slug: Option<String>,
        cause: String,
    },

    #[error("Content fetch failed for source '{source_name}'")]
    FetchFailed { source_name: String, cause: String },

    #[error("Configuration error: {message}")]
    Config {
        message: String,
        path: Option<String>,
    },

    #[error("Page prerenderer '{page_type}' failed: {cause}")]
    PagePrerendererFailed { page_type: String, cause: String },
}

impl PublishError {
    pub fn missing_field(field: impl Into<String>, slug: impl Into<String>) -> Self {
        let field_str = field.into();
        Self::MissingField {
            suggestion: suggest_fix_for_field(&field_str),
            field: field_str,
            slug: slug.into(),
            source_path: None,
        }
    }

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

    pub fn provider_failed(provider_id: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::ProviderFailed {
            provider_id: provider_id.into(),
            cause: cause.into(),
            suggestion: None,
        }
    }

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

    pub fn fetch_failed(source_name: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::FetchFailed {
            source_name: source_name.into(),
            cause: cause.into(),
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            path: None,
        }
    }

    pub fn page_prerenderer_failed(page_type: impl Into<String>, cause: impl Into<String>) -> Self {
        Self::PagePrerendererFailed {
            page_type: page_type.into(),
            cause: cause.into(),
        }
    }

    pub fn location(&self) -> Option<String> {
        match self {
            Self::MissingField { source_path, .. } => {
                source_path.as_ref().map(|p| p.display().to_string())
            },
            Self::Config { path, .. } => path.clone(),
            _ => None,
        }
    }

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

fn suggest_fix_for_field(field: &str) -> Option<String> {
    match field {
        "image" | "cover_image" => {
            Some("Add 'image: \"/files/images/placeholder.svg\"' to frontmatter".to_string())
        },
        "published_at" | "date" | "created_at" | "published_at/date/created_at" => {
            Some("Add 'date: YYYY-MM-DD' to frontmatter".to_string())
        },
        "author" => Some(
            "Add 'author: Your Name' to frontmatter or set metadata.default_author in config"
                .to_string(),
        ),
        "title" => Some("Add 'title: Your Title' to frontmatter".to_string()),
        "slug" => Some("Add 'slug: your-slug' to frontmatter".to_string()),
        "content_type" => {
            Some("Ensure content has a valid 'kind' field in frontmatter".to_string())
        },
        field if field.starts_with("organization.") => Some(format!(
            "Add '{}' under metadata.structured_data.organization in content.yaml",
            field.strip_prefix("organization.").unwrap_or(field)
        )),
        field if field.starts_with("article.") => Some(format!(
            "Add '{}' under metadata.structured_data.article in content.yaml",
            field.strip_prefix("article.").unwrap_or(field)
        )),
        field if field.starts_with("branding.") => Some(format!(
            "Add '{}' under branding in web.yaml",
            field.strip_prefix("branding.").unwrap_or(field)
        )),
        _ => None,
    }
}
