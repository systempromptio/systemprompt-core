//! Whether this build links live OTLP export.
//!
//! A profile that names a collector (`observability.otlp`) is configuration
//! the binary must honour: a build without the `otel` feature has no
//! exporter, so [`ExportCapability::admit`] refuses the block with
//! [`OtlpExportUnavailable`] instead of letting the spans drop unseen.
//! Startup validation applies it to the booting profile with
//! [`ExportCapability::COMPILED`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::OtlpExportConfig;

/// Whether a build carries the OTLP exporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportCapability {
    Linked,
    NotLinked,
}

/// `observability.otlp` names a collector this build cannot export to.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error(
    "observability.otlp names the collector {endpoint}, but this build was compiled without \
     the `otel` feature and cannot export to it; build with the `otel` feature (the \
     `systemprompt` facade's `otel`, included in `reference`) or remove the observability.otlp \
     block"
)]
pub struct OtlpExportUnavailable {
    pub endpoint: String,
}

impl ExportCapability {
    pub const COMPILED: Self = compiled::CAPABILITY;

    pub fn admit(self, otlp: Option<&OtlpExportConfig>) -> Result<(), OtlpExportUnavailable> {
        match (self, otlp) {
            (Self::NotLinked, Some(otlp)) => Err(OtlpExportUnavailable {
                endpoint: otlp.endpoint.clone(),
            }),
            _ => Ok(()),
        }
    }
}

#[cfg(feature = "otel")]
mod compiled {
    pub(super) const CAPABILITY: super::ExportCapability = super::ExportCapability::Linked;
}

#[cfg(not(feature = "otel"))]
mod compiled {
    pub(super) const CAPABILITY: super::ExportCapability = super::ExportCapability::NotLinked;
}
