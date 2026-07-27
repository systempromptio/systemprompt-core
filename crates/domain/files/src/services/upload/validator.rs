//! Upload type and size policy.
//!
//! [`FileValidator`] enforces the configured size limit, rejects a blocklist of
//! executable/script MIME types, and maps allowed types to a [`FileCategory`]
//! used for storage layout and extension resolution. [`FileValidationError`]
//! reports each rejection reason.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::FileUploadConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileValidationError {
    #[error("File uploads are disabled")]
    UploadsDisabled,

    #[error("File size {size} bytes exceeds maximum allowed {max} bytes")]
    FileTooLarge { size: u64, max: u64 },

    #[error("File type '{mime_type}' is not allowed")]
    TypeNotAllowed { mime_type: String },

    #[error("File type '{mime_type}' is blocked for security reasons")]
    TypeBlocked { mime_type: String },

    #[error("File category '{category}' is disabled in configuration")]
    CategoryDisabled { category: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Image,
    Document,
    Audio,
    Video,
}

impl FileCategory {
    pub const fn storage_subdir(&self) -> &'static str {
        match self {
            Self::Image => "images",
            Self::Document => "documents",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }

    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Document => "document",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileValidator {
    config: FileUploadConfig,
}

impl FileValidator {
    const IMAGE_TYPES: &'static [&'static str] = &[
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "image/svg+xml",
        "image/bmp",
        "image/tiff",
        "image/x-icon",
        "image/vnd.microsoft.icon",
    ];

    const DOCUMENT_TYPES: &'static [&'static str] = &[
        "application/pdf",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.ms-powerpoint",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "text/plain",
        "text/csv",
        "text/markdown",
        "text/html",
        "application/json",
        "application/xml",
        "text/xml",
        "application/rtf",
    ];

    const AUDIO_TYPES: &'static [&'static str] = &[
        "audio/mpeg",
        "audio/mp3",
        "audio/wav",
        "audio/wave",
        "audio/x-wav",
        "audio/ogg",
        "audio/webm",
        "audio/aac",
        "audio/flac",
        "audio/mp4",
        "audio/x-m4a",
    ];

    const VIDEO_TYPES: &'static [&'static str] = &[
        "video/mp4",
        "video/webm",
        "video/ogg",
        "video/quicktime",
        "video/x-msvideo",
        "video/x-matroska",
    ];

    const BLOCKED_TYPES: &'static [&'static str] = &[
        "application/x-executable",
        "application/x-msdos-program",
        "application/x-msdownload",
        "application/x-sh",
        "application/x-shellscript",
        "application/x-csh",
        "application/x-bash",
        "application/bat",
        "application/x-bat",
        "application/x-msi",
        "application/vnd.microsoft.portable-executable",
        "application/x-dosexec",
        "application/x-python-code",
        "application/javascript",
        "text/javascript",
        "application/x-httpd-php",
        "application/x-php",
        "text/x-php",
        "application/x-perl",
        "text/x-perl",
        "application/x-ruby",
        "text/x-ruby",
        "application/java-archive",
        "application/x-java-class",
    ];

    pub const fn new(config: FileUploadConfig) -> Self {
        Self { config }
    }

    pub fn validate(
        &self,
        mime_type: &str,
        size_bytes: u64,
    ) -> Result<FileCategory, FileValidationError> {
        if !self.config.enabled {
            return Err(FileValidationError::UploadsDisabled);
        }

        if size_bytes > self.config.max_file_size_bytes {
            return Err(FileValidationError::FileTooLarge {
                size: size_bytes,
                max: self.config.max_file_size_bytes,
            });
        }

        let normalized_mime = systemprompt_models::mime::essence_of(mime_type);

        if Self::BLOCKED_TYPES.contains(&normalized_mime.as_str()) {
            return Err(FileValidationError::TypeBlocked {
                mime_type: mime_type.to_owned(),
            });
        }

        let category = Self::categorize_mime_type(&normalized_mime)?;

        if !self.is_category_allowed(&category) {
            return Err(FileValidationError::CategoryDisabled {
                category: category.display_name().to_owned(),
            });
        }

        Ok(category)
    }

    fn categorize_mime_type(mime_type: &str) -> Result<FileCategory, FileValidationError> {
        if Self::IMAGE_TYPES.contains(&mime_type) {
            return Ok(FileCategory::Image);
        }

        if Self::DOCUMENT_TYPES.contains(&mime_type) {
            return Ok(FileCategory::Document);
        }

        if Self::AUDIO_TYPES.contains(&mime_type) {
            return Ok(FileCategory::Audio);
        }

        if Self::VIDEO_TYPES.contains(&mime_type) {
            return Ok(FileCategory::Video);
        }

        Err(FileValidationError::TypeNotAllowed {
            mime_type: mime_type.to_owned(),
        })
    }

    const fn is_category_allowed(&self, category: &FileCategory) -> bool {
        match category {
            FileCategory::Image => self.config.allowed_types.images,
            FileCategory::Document => self.config.allowed_types.documents,
            FileCategory::Audio => self.config.allowed_types.audio,
            FileCategory::Video => self.config.allowed_types.video,
        }
    }

    pub fn get_extension(mime_type: &str, filename: Option<&str>) -> String {
        if let Some(name) = filename
            && let Some(ext) = name.rsplit('.').next()
            && !ext.is_empty()
            && ext.len() <= 10
            && ext != name
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return ext.to_lowercase();
        }

        systemprompt_models::mime::extension_for(mime_type)
            .unwrap_or("bin")
            .to_owned()
    }
}
