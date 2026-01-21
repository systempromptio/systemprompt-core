use async_trait::async_trait;
use systemprompt_traits::{
    FileUploadInput, FileUploadProvider, FileUploadProviderError, FileUploadResult,
    UploadedFileInfo,
};

use super::upload::{FileUploadRequest, FileUploadService};

#[async_trait]
impl FileUploadProvider for FileUploadService {
    fn is_enabled(&self) -> bool {
        FileUploadService::is_enabled(self)
    }

    async fn upload_file(&self, input: FileUploadInput) -> FileUploadResult<UploadedFileInfo> {
        let mut builder =
            FileUploadRequest::builder(&input.mime_type, &input.bytes_base64, input.context_id);

        if let Some(name) = input.name {
            builder = builder.with_name(&name);
        }

        if let Some(user_id) = input.user_id {
            builder = builder.with_user_id(user_id);
        }

        if let Some(session_id) = input.session_id {
            builder = builder.with_session_id(session_id);
        }

        if let Some(trace_id) = input.trace_id {
            builder = builder.with_trace_id(trace_id);
        }

        let request = builder.build();

        let uploaded = FileUploadService::upload_file(self, request)
            .await
            .map_err(|e| FileUploadProviderError::StorageError(e.to_string()))?;

        Ok(UploadedFileInfo {
            file_id: uploaded.file_id,
            public_url: uploaded.public_url,
            size_bytes: Some(uploaded.size_bytes),
        })
    }
}
