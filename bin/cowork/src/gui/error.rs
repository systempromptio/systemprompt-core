use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum GuiError {
    #[error("io: {0}")]
    Io(String),
    #[error("http: {0}")]
    Http(String),
    #[error("auth: {0}")]
    Auth(String),
    #[error("sync: {0}")]
    Sync(String),
    #[error("tray: {0}")]
    Tray(String),
    #[error("icon: {0}")]
    Icon(String),
    #[error("window: {0}")]
    Window(String),
    #[error("profile: {0}")]
    Profile(String),
}

impl From<std::io::Error> for GuiError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<crate::auth::setup::SetupError> for GuiError {
    fn from(e: crate::auth::setup::SetupError) -> Self {
        Self::Auth(e.to_string())
    }
}

impl From<crate::sync::SyncError> for GuiError {
    fn from(e: crate::sync::SyncError) -> Self {
        Self::Sync(e.to_string())
    }
}

pub type GuiResult<T> = std::result::Result<T, GuiError>;
