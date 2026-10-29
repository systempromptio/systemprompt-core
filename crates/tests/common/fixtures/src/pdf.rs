//! Reads the PDFs the document renderer writes back with `lopdf`: the page
//! text in page order, the info dictionary and the fonts the file declares.

use std::collections::BTreeMap;

use lopdf::{Document, Object};

#[derive(Debug, Clone)]
pub struct ParsedPdf {
    pub pages: Vec<String>,
    document: Document,
}

pub fn parse_pdf(bytes: &[u8]) -> ParsedPdf {
    assert!(bytes.starts_with(b"%PDF-1."), "missing PDF header");
    let document = Document::load_mem(bytes).expect("a well-formed PDF");
    let numbers: BTreeMap<u32, lopdf::ObjectId> = document.get_pages();
    let pages = numbers
        .keys()
        .map(|number| document.extract_text(&[*number]).expect("page text"))
        .collect();
    ParsedPdf { pages, document }
}

impl ParsedPdf {
    pub fn text(&self) -> String {
        self.pages.join("\n")
    }

    pub fn info(&self, key: &str) -> Option<String> {
        let info = self
            .document
            .trailer
            .get(b"Info")
            .ok()?
            .as_reference()
            .ok()?;
        let value = self
            .document
            .get_dictionary(info)
            .ok()?
            .get(key.as_bytes())
            .ok()?;
        let bytes = value.as_str().ok()?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn has_font(&self, base_font: &str) -> bool {
        self.document.objects.values().any(|object| {
            let Object::Dictionary(dict) = object else {
                return false;
            };
            dict.get(b"BaseFont")
                .ok()
                .and_then(|name| name.as_name().ok())
                .is_some_and(|name| name == base_font.as_bytes())
        })
    }
}
