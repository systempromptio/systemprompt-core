use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("auth: {0}")]
    Auth(#[from] crate::auth::setup::SetupError),

    #[error("sync: {0}")]
    Sync(#[from] crate::sync::SyncError),

    #[error("gateway: {0}")]
    Gateway(#[from] crate::gateway::GatewayError),

    #[error("profile: {context}: {source}")]
    Profile {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("tray menu: {0}")]
    TrayMenu(#[from] muda::Error),

    #[error("tray build: {0}")]
    TrayBuild(#[from] tray_icon::Error),

    #[error("icon decode: {0}")]
    IconImage(#[from] image::ImageError),

    #[error("icon: {0}")]
    Icon(#[from] tray_icon::BadIcon),

    #[error("window: {context}: {source}")]
    Window {
        context: String,
        #[source]
        source: WindowError,
    },
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error(transparent)]
    Os(#[from] winit::error::OsError),
    #[error(transparent)]
    Wry(#[from] wry::Error),
}

pub type GuiResult<T> = std::result::Result<T, GuiError>;
