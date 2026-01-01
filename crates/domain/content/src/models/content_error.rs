#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error(
        "Missing organization config: '{field}' in content.yaml under \
         metadata.structured_data.organization"
    )]
    MissingOrgConfig { field: String },

    #[error(
        "Missing article config: '{field}' in content.yaml under metadata.structured_data.article"
    )]
    MissingArticleConfig { field: String },

    #[error("Invalid content: {message}")]
    InvalidContent { message: String },

    #[error("Missing branding config: '{field}' in web.yaml under branding")]
    MissingBrandingConfig { field: String },
}

impl ContentError {
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    pub fn missing_org_config(field: impl Into<String>) -> Self {
        Self::MissingOrgConfig {
            field: field.into(),
        }
    }

    pub fn missing_article_config(field: impl Into<String>) -> Self {
        Self::MissingArticleConfig {
            field: field.into(),
        }
    }

    pub fn invalid_content(message: impl Into<String>) -> Self {
        Self::InvalidContent {
            message: message.into(),
        }
    }

    pub fn missing_branding_config(field: impl Into<String>) -> Self {
        Self::MissingBrandingConfig {
            field: field.into(),
        }
    }
}
