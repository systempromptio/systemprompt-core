//! Core's embedded default catalog.
//!
//! Five files under `default/`, in the same shape and with the same names an
//! install keeps under `services/ai/`. The setup wizard and cloud-init write
//! [`DefaultCatalog::FILES`] into a new install verbatim; the parity tests
//! read [`DefaultCatalog::document`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::document::CatalogDocument;
use super::error::CatalogDocumentError;

#[derive(Debug, Clone, Copy)]
pub struct DefaultCatalog;

impl DefaultCatalog {
    pub const FILES: [(&'static str, &'static str); 5] = [
        ("credentials.yaml", include_str!("default/credentials.yaml")),
        ("providers.yaml", include_str!("default/providers.yaml")),
        ("models.yaml", include_str!("default/models.yaml")),
        ("deployments.yaml", include_str!("default/deployments.yaml")),
        ("gateway.yaml", include_str!("default/gateway.yaml")),
    ];

    pub fn document() -> Result<CatalogDocument, CatalogDocumentError> {
        let mut merged = CatalogDocument::default();
        for (name, text) in Self::FILES {
            merged.merge(CatalogDocument::from_yaml_str(name, text)?)?;
        }
        Ok(merged)
    }
}
