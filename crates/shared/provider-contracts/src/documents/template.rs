//! The [`DocumentTemplateProvider`] seam and its compile-time registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::DocumentTemplateId;

use super::format::DocumentFormat;

/// Where a template's bytes come from.
///
/// `ServicesPath` is relative to the services directory and is resolved by
/// the caller that owns the services path; a provider never reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateSource {
    Embedded(&'static [u8]),
    ServicesPath(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TemplatePlaceholder {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTemplate {
    pub id: DocumentTemplateId,
    pub format: DocumentFormat,
    pub name: String,
    pub description: String,
    pub source: TemplateSource,
    pub placeholders: Vec<TemplatePlaceholder>,
}

/// Contributes document templates.
///
/// When two providers publish the same template id, the template from the
/// provider with the higher `priority` is used.
pub trait DocumentTemplateProvider: Send + Sync + std::fmt::Debug {
    fn provider_id(&self) -> &'static str;

    fn templates(&self) -> Vec<DocumentTemplate>;

    fn priority(&self) -> u32 {
        100
    }
}

/// One compile-time template provider registration, submitted by
/// [`register_document_template_provider!
/// `](crate::register_document_template_provider).
#[derive(Debug, Clone, Copy)]
pub struct DocumentTemplateProviderRegistration {
    pub factory: fn() -> Arc<dyn DocumentTemplateProvider>,
}

inventory::collect!(DocumentTemplateProviderRegistration);

#[macro_export]
macro_rules! register_document_template_provider {
    ($factory:path) => {
        $crate::__inventory::submit! {
            $crate::documents::DocumentTemplateProviderRegistration { factory: $factory }
        }
    };
}

pub fn registered_template_providers() -> impl Iterator<Item = Arc<dyn DocumentTemplateProvider>> {
    let mut providers: Vec<Arc<dyn DocumentTemplateProvider>> =
        inventory::iter::<DocumentTemplateProviderRegistration>
            .into_iter()
            .map(|registration| (registration.factory)())
            .collect();
    providers.sort_by_key(|provider| std::cmp::Reverse(provider.priority()));
    providers.into_iter()
}
