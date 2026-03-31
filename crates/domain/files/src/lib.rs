pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod extension;
pub(crate) mod jobs;
pub(crate) mod models;
pub(crate) mod repository;
pub(crate) mod services;

pub use extension::FilesExtension;

pub use config::{
    AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfig, FilesConfigValidator,
    FilesConfigYaml,
};
pub use jobs::FileIngestionJob;
pub use models::{
    AudioMetadata, ContentFile, DocumentMetadata, File, FileChecksums, FileMetadata, FileRole,
    ImageGenerationInfo, ImageMetadata, TypeSpecificMetadata, VideoMetadata,
};
pub use repository::{FileRepository, FileStats, InsertFileRequest};
pub use services::{
    AiService, ContentService, FileCategory, FileService, FileUploadError, FileUploadRequest,
    FileUploadRequestBuilder, FileUploadService, FileValidationError, FileValidator,
    FilesAiPersistenceProvider, UploadedFile,
};
