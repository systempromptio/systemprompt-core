use serde::{Deserialize, Serialize};

const DEFAULT_URL_PREFIX: &str = "/files";
pub(super) const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilePersistenceMode {
    #[default]
    ContextScoped,
    UserLibrary,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct AllowedFileTypes {
    pub images: bool,
    pub documents: bool,
    pub audio: bool,
    pub video: bool,
}

impl Default for AllowedFileTypes {
    fn default() -> Self {
        Self {
            images: true,
            documents: true,
            audio: true,
            video: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FileUploadConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
    #[serde(default)]
    pub persistence_mode: FilePersistenceMode,
    #[serde(default)]
    pub allowed_types: AllowedFileTypes,
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_file_size() -> u64 {
    DEFAULT_MAX_FILE_SIZE_BYTES
}

impl Default for FileUploadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
            persistence_mode: FilePersistenceMode::default(),
            allowed_types: AllowedFileTypes::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesConfigYaml {
    #[serde(default = "default_url_prefix")]
    pub url_prefix: String,
    #[serde(default)]
    pub upload: FileUploadConfig,
}

fn default_url_prefix() -> String {
    DEFAULT_URL_PREFIX.to_string()
}

impl Default for FilesConfigYaml {
    fn default() -> Self {
        Self {
            url_prefix: default_url_prefix(),
            upload: FileUploadConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FilesConfigWrapper {
    #[serde(default)]
    pub files: FilesConfigYaml,
}
