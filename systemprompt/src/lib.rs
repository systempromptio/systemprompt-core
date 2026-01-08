#![allow(clippy::doc_markdown)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod traits {
    pub use systemprompt_traits::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod models {
    pub use systemprompt_models::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod identifiers {
    pub use systemprompt_identifiers::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod extension {
    pub use systemprompt_extension::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod template_provider {
    pub use systemprompt_template_provider::*;
}

#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
pub mod database {
    pub use systemprompt_core_database::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod logging {
    pub use systemprompt_core_logging::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod config {
    pub use systemprompt_core_config::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod loader {
    pub use systemprompt_loader::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod events {
    pub use systemprompt_core_events::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod client {
    pub use systemprompt_client::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod security {
    pub use systemprompt_core_security::*;
}

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod system {
    pub use systemprompt_runtime::*;
}

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api {
    pub use systemprompt_core_api::*;
}

#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli {
    pub use systemprompt_cli::{run, CliConfig, ColorMode, OutputFormat, VerbosityLevel};
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent {
    pub use systemprompt_core_agent::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai {
    pub use systemprompt_core_ai::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    pub use systemprompt_core_mcp::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth {
    pub use systemprompt_core_oauth::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users {
    pub use systemprompt_core_users::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content {
    pub use systemprompt_core_content::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod analytics {
    pub use systemprompt_core_analytics::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod scheduler {
    pub use systemprompt_core_scheduler::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod files {
    pub use systemprompt_core_files::*;
}

#[cfg(feature = "sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
pub mod sync {
    pub use systemprompt_sync::*;
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod cloud {
    pub use systemprompt_cloud::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod profile {
    pub use systemprompt_models::profile::{
        CloudConfig, CloudValidationMode, Profile, ProfileStyle,
    };
    pub use systemprompt_models::profile_bootstrap::{ProfileBootstrap, ProfileBootstrapError};
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};

mod prelude;

pub use crate::prelude::*;
