//! Tests for `plugins show` detail projection — mapping an `Extension`'s
//! metadata, templates, schemas, and roles into `ExtensionDetailOutput`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use systemprompt_cli::plugins::show::build_detail_output;
use systemprompt_cli::plugins::types::ExtensionSource;
use systemprompt_extension::prelude::{TemplateDefinition, TemplateProvider};
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRole, SchemaDefinition};

struct StubTemplates;

impl TemplateProvider for StubTemplates {
    fn templates(&self) -> Vec<TemplateDefinition> {
        let mut def = TemplateDefinition::embedded("landing", "<html></html>");
        def.content_types = vec!["page".to_owned(), "post".to_owned()];
        vec![def]
    }

    fn provider_id(&self) -> &'static str {
        "stub-templates"
    }
}

struct StubExtension;

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "stub-ext",
            name: "Stub Extension",
            version: "2.3.4",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new("widgets", "CREATE TABLE widgets (id TEXT)")
                .with_required_columns(vec!["id".to_owned()]),
        ]
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("stub")
    }

    fn template_providers(&self) -> Vec<Arc<dyn TemplateProvider>> {
        vec![Arc::new(StubTemplates)]
    }

    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec!["files/stub"]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["core"]
    }

    fn roles(&self) -> Vec<ExtensionRole> {
        vec![ExtensionRole {
            name: "stub_admin".to_owned(),
            display_name: "Stub Admin".to_owned(),
            description: "Administers stubs".to_owned(),
            permissions: vec!["stub:write".to_owned()],
        }]
    }
}

struct BareExtension;

impl Extension for BareExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "bare",
            name: "Bare",
            version: "0.0.1",
        }
    }
}

#[test]
fn build_detail_output_projects_all_populated_sections() {
    let out = build_detail_output(&StubExtension);

    assert_eq!(out.id.as_str(), "stub-ext");
    assert_eq!(out.name, "Stub Extension");
    assert_eq!(out.version, "2.3.4");
    assert_eq!(out.priority, 100);
    assert!(matches!(out.source, ExtensionSource::Compiled));
    assert_eq!(out.dependencies, vec!["core".to_owned()]);
    assert_eq!(out.config_prefix.as_deref(), Some("stub"));
    assert_eq!(out.storage_paths, vec!["files/stub".to_owned()]);

    assert_eq!(out.templates.len(), 1);
    assert_eq!(out.templates[0].name, "landing");
    assert_eq!(out.templates[0].description, "page, post");

    assert_eq!(out.schemas.len(), 1);
    assert_eq!(out.schemas[0].table, "widgets");
    assert_eq!(out.schemas[0].source, "inline");
    assert_eq!(out.schemas[0].required_columns, vec!["id".to_owned()]);

    assert_eq!(out.roles.len(), 1);
    assert_eq!(out.roles[0].name, "stub_admin");
    assert_eq!(out.roles[0].permissions, vec!["stub:write".to_owned()]);
}

#[test]
fn build_detail_output_defaults_to_empty_sections() {
    let out = build_detail_output(&BareExtension);

    assert_eq!(out.id.as_str(), "bare");
    assert!(out.jobs.is_empty());
    assert!(out.templates.is_empty());
    assert!(out.schemas.is_empty());
    assert!(out.tools.is_empty());
    assert!(out.roles.is_empty());
    assert!(out.llm_providers.is_empty());
    assert!(out.config_prefix.is_none());
}
