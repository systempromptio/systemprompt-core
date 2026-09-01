//! GUI error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("not authenticated")]
    NotAuthenticated,

    // Why: an operation that was stopped before it concluded produced no
    // finding at all. Modelled as its own variant so no caller has to sniff
    // a message string to tell "the user pressed Cancel" from "this failed".
    #[error("cancelled")]
    Cancelled,

    #[error("auth: {0}")]
    Auth(#[from] crate::auth::setup::SetupError),

    #[error("sync: {0}")]
    Sync(#[from] crate::sync::SyncError),

    #[error("gateway: {0}")]
    Gateway(#[from] crate::gateway::GatewayError),

    #[error("update: {0}")]
    Update(#[from] crate::update::UpdateError),

    #[error("config: {0}")]
    Config(#[from] crate::config::ConfigWriteError),

    #[error("install: {0}")]
    Install(#[from] crate::install::InstallError),

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

impl GuiError {
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Auth(crate::auth::setup::SetupError::Cancelled)
        )
    }
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error(transparent)]
    Os(#[from] winit::error::RequestError),
    #[error(transparent)]
    Wry(#[from] wry::Error),
}

pub type GuiResult<T> = Result<T, GuiError>;
