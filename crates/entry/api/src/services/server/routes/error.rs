//! Failures assembling the route tree at startup.
//!
//! [`RouteMountError`] wraps the extension loader's own errors and adds the
//! startup components (metrics recorder, gateway, profile, extension config)
//! whose failures carry an underlying cause the loader error cannot hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{ExtensionConfigError, LoaderError};
use systemprompt_traits::BoxedSource;

#[derive(Debug, thiserror::Error)]
pub(crate) enum RouteMountError {
    #[error(transparent)]
    Loader(#[from] LoaderError),

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
        source: ExtensionConfigError,
    },
}

impl RouteMountError {
    pub(super) fn initialization(component: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Initialization {
            component,
            source: source.into(),
        }
    }
}
