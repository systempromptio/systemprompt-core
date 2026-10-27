//! The blank templates core ships.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::DocumentTemplateId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, DocumentTemplate, DocumentTemplateProvider, TemplatePlaceholder,
    TemplateSource,
};
use systemprompt_provider_contracts::register_document_template_provider;

pub const CORE_TEMPLATE_PRIORITY: u32 = 100;

const BLANK_TEXT: &[u8] = b"\n";
const BLANK_MARKDOWN: &[u8] = b"# {{title}}\n\n";
const BLANK_HTML: &[u8] = b"<!doctype html><html><head><meta charset=\"utf-8\"><title>{{title}}</title></head><body><h1>{{title}}</h1><p></p></body></html>";

/// Publishes the core blank document templates at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreTemplateProvider;

impl DocumentTemplateProvider for CoreTemplateProvider {
    fn provider_id(&self) -> &'static str {
        "core"
    }

    fn templates(&self) -> Vec<DocumentTemplate> {
        vec![
            blank(
                "blank-text",
                DocumentFormat::Text,
                "Blank text file",
                BLANK_TEXT,
                false,
            ),
            blank(
                "blank-markdown",
                DocumentFormat::Markdown,
                "Blank Markdown document",
                BLANK_MARKDOWN,
                true,
            ),
            blank(
                "blank-html",
                DocumentFormat::Html,
                "Blank HTML document",
                BLANK_HTML,
                true,
            ),
        ]
    }

    fn priority(&self) -> u32 {
        CORE_TEMPLATE_PRIORITY
    }
}

fn factory() -> Arc<dyn DocumentTemplateProvider> {
    Arc::new(CoreTemplateProvider)
}

register_document_template_provider!(factory);

fn blank(
    id: &str,
    format: DocumentFormat,
    name: &str,
    bytes: &'static [u8],
    titled: bool,
) -> DocumentTemplate {
    let placeholders = if titled {
        vec![TemplatePlaceholder {
            key: "title".to_owned(),
            label: "Title".to_owned(),
            default: Some("Untitled".to_owned()),
        }]
    } else {
        Vec::new()
    };
    DocumentTemplate {
        id: DocumentTemplateId::new(id),
        format,
        name: name.to_owned(),
        description: format!("An empty {} document", format.as_str()),
        source: TemplateSource::Embedded(bytes),
        placeholders,
    }
}

/// A core template whose bytes are produced by a function instead of being
/// embedded, for formats whose blank is a generated archive.
#[derive(Debug, Clone)]
pub struct GeneratedBlank {
    pub template: DocumentTemplate,
    pub generate: fn() -> Result<Vec<u8>, CodecError>,
}

#[must_use]
pub fn generated_blank(
    id: &str,
    format: DocumentFormat,
    name: &str,
    generate: fn() -> Result<Vec<u8>, CodecError>,
) -> GeneratedBlank {
    GeneratedBlank {
        template: blank(id, format, name, &[], false),
        generate,
    }
}

#[cfg(feature = "office")]
#[must_use]
pub fn office_blanks() -> Vec<GeneratedBlank> {
    use crate::documents::codecs::{docx, odf, pptx, xlsx};
    vec![
        generated_blank(
            "blank-docx",
            DocumentFormat::Docx,
            "Blank Word document",
            docx::blank_docx,
        ),
        generated_blank(
            "blank-xlsx",
            DocumentFormat::Xlsx,
            "Blank Excel workbook",
            xlsx::blank_xlsx,
        ),
        generated_blank(
            "blank-pptx",
            DocumentFormat::Pptx,
            "Blank PowerPoint presentation",
            pptx::blank_pptx,
        ),
        generated_blank(
            "blank-odt",
            DocumentFormat::Odt,
            "Blank OpenDocument text",
            odf::blank_odt,
        ),
        generated_blank(
            "blank-ods",
            DocumentFormat::Ods,
            "Blank OpenDocument spreadsheet",
            odf::blank_ods,
        ),
        generated_blank(
            "blank-odp",
            DocumentFormat::Odp,
            "Blank OpenDocument presentation",
            odf::blank_odp,
        ),
    ]
}

#[cfg(not(feature = "office"))]
#[must_use]
pub const fn office_blanks() -> Vec<GeneratedBlank> {
    Vec::new()
}
