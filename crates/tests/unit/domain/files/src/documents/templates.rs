use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::templates::{
    CoreTemplateProvider, TemplateCatalogError, TemplateError, TemplateRegistry, load_catalog,
};
use systemprompt_identifiers::DocumentTemplateId;
use systemprompt_provider_contracts::documents::{
    DocumentFormat, DocumentTemplate, DocumentTemplateProvider, TemplateParams, TemplateSource,
};
use systemprompt_provider_contracts::register_document_template_provider;

#[derive(Debug)]
struct ProbeProvider;

impl DocumentTemplateProvider for ProbeProvider {
    fn provider_id(&self) -> &'static str {
        "probe"
    }

    fn templates(&self) -> Vec<DocumentTemplate> {
        vec![DocumentTemplate {
            id: DocumentTemplateId::new("probe-text"),
            format: DocumentFormat::Text,
            name: "Probe".to_owned(),
            description: String::new(),
            source: TemplateSource::Embedded(b"probe {{who}}\n"),
            placeholders: vec![
                systemprompt_provider_contracts::documents::TemplatePlaceholder {
                    key: "who".to_owned(),
                    label: "Who".to_owned(),
                    default: None,
                },
            ],
        }]
    }

    fn priority(&self) -> u32 {
        50
    }
}

fn probe_factory() -> Arc<dyn DocumentTemplateProvider> {
    Arc::new(ProbeProvider)
}

register_document_template_provider!(probe_factory);

fn fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/documents/templates")
        .join(name)
}

fn tid(value: &str) -> DocumentTemplateId {
    DocumentTemplateId::new(value)
}

fn core_only() -> TemplateRegistry {
    let core: Arc<dyn DocumentTemplateProvider> = Arc::new(CoreTemplateProvider);
    TemplateRegistry::from_sources([core], Vec::new())
}

fn instantiate_str(registry: &TemplateRegistry, id: &str, params: &TemplateParams) -> String {
    let made = registry
        .instantiate(&tid(id), params, &CodecRegistry::discover())
        .expect("instantiate");
    String::from_utf8(made.bytes).expect("utf8")
}

#[test]
fn catalog_loads_and_resolves_relative_file() {
    let catalog = load_catalog(&fixture_root("valid")).expect("catalog");
    assert_eq!(catalog.len(), 1);
    let entry = &catalog[0];
    assert_eq!(entry.priority, 200);
    assert_eq!(entry.template.format, DocumentFormat::Markdown);
    assert_eq!(entry.template.placeholders.len(), 2);
    let TemplateSource::ServicesPath(path) = &entry.template.source else {
        panic!("expected a services path source");
    };
    assert!(path.is_absolute());
    assert!(path.ends_with("documents/templates/files/memo.md"));
}

#[test]
fn missing_catalog_dir_is_empty() {
    let catalog = load_catalog(&fixture_root("does-not-exist")).expect("catalog");
    assert!(catalog.is_empty());
}

#[test]
fn duplicate_id_names_both_paths() {
    let err = load_catalog(&fixture_root("duplicate")).expect_err("duplicate");
    let TemplateCatalogError::DuplicateId { id, first, second } = err else {
        panic!("expected DuplicateId, got {err:?}");
    };
    assert_eq!(id.as_str(), "twin");
    assert!(first.ends_with("a.yaml"));
    assert!(second.ends_with("b.yaml"));
}

#[test]
fn unknown_format_is_rejected() {
    let err = load_catalog(&fixture_root("unknown")).expect_err("unknown");
    assert!(
        matches!(&err, TemplateCatalogError::UnknownFormat { source, .. } if source.value == "rtf"),
        "{err:?}"
    );
}

#[test]
fn malformed_yaml_is_rejected() {
    let err = load_catalog(&fixture_root("malformed")).expect_err("malformed");
    assert!(matches!(err, TemplateCatalogError::Yaml { .. }), "{err:?}");
}

#[test]
fn catalog_priority_overrides_provider_template() {
    let catalog = load_catalog(&fixture_root("valid")).expect("catalog");
    let core: Arc<dyn DocumentTemplateProvider> = Arc::new(CoreTemplateProvider);
    let registry = TemplateRegistry::from_sources([core], catalog);
    let template = registry.get(&tid("blank-markdown")).expect("template");
    assert_eq!(template.name, "Branded memo");
    let mut params = TemplateParams::default();
    params.values.insert("author".to_owned(), "Ada".to_owned());
    assert_eq!(
        instantiate_str(&registry, "blank-markdown", &params),
        "# Untitled\n\nPrepared by Ada.\n"
    );
}

#[test]
fn blank_markdown_fills_title() {
    let mut params = TemplateParams::default();
    params.values.insert("title".to_owned(), "Plan".to_owned());
    let made = instantiate_str(&core_only(), "blank-markdown", &params);
    assert!(made.starts_with("# Plan"), "{made}");
}

#[test]
fn blank_markdown_applies_default_title() {
    let made = instantiate_str(&core_only(), "blank-markdown", &TemplateParams::default());
    assert!(made.starts_with("# Untitled"), "{made}");
}

#[test]
fn missing_required_placeholder_is_an_error() {
    let catalog = load_catalog(&fixture_root("valid")).expect("catalog");
    let registry = TemplateRegistry::from_sources(Vec::new(), catalog);
    let err = registry
        .instantiate(
            &tid("blank-markdown"),
            &TemplateParams::default(),
            &CodecRegistry::discover(),
        )
        .expect_err("missing");
    assert!(
        matches!(&err, TemplateError::MissingPlaceholder { key } if key == "author"),
        "{err:?}"
    );
}

#[test]
fn unknown_template_is_not_found() {
    let err = core_only()
        .instantiate(
            &tid("nope"),
            &TemplateParams::default(),
            &CodecRegistry::discover(),
        )
        .expect_err("not found");
    assert!(matches!(err, TemplateError::NotFound { .. }), "{err:?}");
}

#[test]
fn list_for_html_contains_blank_html() {
    let registry = core_only();
    let html = registry.list_for(DocumentFormat::Html);
    assert!(html.iter().any(|t| t.id.as_str() == "blank-html"));
    assert!(html.iter().all(|t| t.format == DocumentFormat::Html));
}

#[test]
fn discover_includes_core_and_macro_registered_provider() {
    let registry = TemplateRegistry::discover(Vec::new());
    assert!(registry.get(&tid("blank-text")).is_some());
    assert!(registry.get(&tid("probe-text")).is_some());
    let mut params = TemplateParams::default();
    params.values.insert("who".to_owned(), "me".to_owned());
    assert_eq!(
        instantiate_str(&registry, "probe-text", &params),
        "probe me\n"
    );
}

#[test]
fn office_blanks_are_listed_and_project() {
    let registry = TemplateRegistry::discover(Vec::new());
    let codecs = CodecRegistry::discover();
    for (id, format) in [
        ("blank-docx", DocumentFormat::Docx),
        ("blank-xlsx", DocumentFormat::Xlsx),
        ("blank-pptx", DocumentFormat::Pptx),
    ] {
        assert!(
            registry
                .list_for(format)
                .iter()
                .any(|t| t.id.as_str() == id),
            "{id} not listed"
        );
        let bytes = registry.load_bytes(&tid(id)).expect("bytes");
        codecs
            .for_format(format)
            .expect("codec")
            .project(&bytes)
            .expect("project");
    }
}

fn shipped_codecs() -> CodecRegistry {
    use systemprompt_files::documents::codecs::docx::DocxCodec;
    use systemprompt_files::documents::codecs::odf::{OdpCodec, OdsCodec, OdtCodec};
    use systemprompt_files::documents::codecs::pptx::PptxCodec;
    use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
    use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec, TextCodec};
    use systemprompt_provider_contracts::documents::DocumentCodec;
    let codecs: Vec<Arc<dyn DocumentCodec>> = vec![
        Arc::new(TextCodec),
        Arc::new(MarkdownCodec),
        Arc::new(HtmlCodec),
        Arc::new(DocxCodec),
        Arc::new(XlsxCodec),
        Arc::new(PptxCodec),
        Arc::new(OdtCodec),
        Arc::new(OdsCodec),
        Arc::new(OdpCodec),
    ];
    CodecRegistry::from_codecs(codecs)
}

#[test]
fn every_registered_template_instantiates_and_projects() {
    let catalog = load_catalog(&fixture_root("valid")).expect("catalog");
    let registry = TemplateRegistry::discover(catalog);
    let codecs = shipped_codecs();
    let templates = registry.list();
    assert!(templates.len() > 9, "{} templates", templates.len());
    for template in templates {
        let mut params = TemplateParams::default();
        for placeholder in template.placeholders.iter().filter(|p| p.default.is_none()) {
            params
                .values
                .insert(placeholder.key.clone(), placeholder.key.clone());
        }
        let made = registry
            .instantiate(&template.id, &params, &codecs)
            .unwrap_or_else(|err| panic!("{} does not instantiate: {err}", template.id));
        codecs
            .for_format(made.format)
            .expect("codec")
            .project(&made.bytes)
            .unwrap_or_else(|err| panic!("{} does not project: {err}", template.id));
    }
}

#[test]
fn a_template_failure_names_the_template_and_the_cause() {
    use systemprompt_files::documents::DocumentsError;
    let id = tid("astound-word");
    let error = DocumentsError::Template {
        template_id: id.clone(),
        source: TemplateError::NotFound { id },
    };
    let message = error.to_string();
    assert!(message.contains("astound-word"), "{message}");
    assert!(message.contains("does not exist"), "{message}");
    assert!(std::error::Error::source(&error).is_some());
}
