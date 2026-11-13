//! Failures assembling the route tree at startup.
//!
//! [`RouteMountError`] wraps the extension loader's own errors and the
//! declaration-mount refusals, and adds the startup components (metrics
//! recorder, gateway, profile, extension config) whose failures carry an
//! underlying cause the loader error cannot hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{ExtensionConfigError, LoaderError};
use systemprompt_traits::BoxedSource;

use crate::services::middleware::MountError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum RouteMountError {
    #[error(transparent)]
    Loader(Box<LoaderError>),

    #[error(transparent)]
    Mount(Box<MountError>),

    #[error("failed to initialise {component}")]
    Initialization {
        component: &'static str,
        #[source]
        source: BoxedSource,
    },

    #[error("extension '{extension}' rejected its configuration")]
    ConfigValidation {
        extension: String,
        #[source]
        source: Box<ExtensionConfigError>,
    },
}

impl From<LoaderError> for RouteMountError {
    fn from(source: LoaderError) -> Self {
        Self::Loader(Box::new(source))
    }
}

impl From<MountError> for RouteMountError {
    fn from(source: MountError) -> Self {
        Self::Mount(Box::new(source))
    }
}

impl RouteMountError {
    pub(super) fn initialization(component: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Initialization {
            component,
            source: source.into(),
        }
    }
}
