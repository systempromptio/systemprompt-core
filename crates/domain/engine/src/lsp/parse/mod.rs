//! Reading protocol JSON into the client's own types. Malformed entries are
//! skipped, never fatal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::types::{Diagnostic, Hover, Location, Position, Range, Severity};
use crate::{paths, uri};

mod symbols;

pub use symbols::{document_symbols, workspace_symbols};

/// One `textDocument/publishDiagnostics` notification, resolved to a local
/// path.
#[derive(Debug)]
pub struct PublishedDiagnostics {
    pub path: PathBuf,
    pub version: Option<i64>,
    pub diagnostics: Vec<Diagnostic>,
}

/// What the server said it supports, reduced to what this client uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerCapabilities {
    pub save: Option<SaveMode>,
    pub hover: bool,
    pub definition: bool,
    pub references: bool,
    pub document_symbol: bool,
    pub workspace_symbol: bool,
}

/// Whether the server wants `didSave`, and with the document text or without
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMode {
    Plain,
    WithText,
}

fn path_of(uri: &str) -> Option<PathBuf> {
    uri::uri_to_path(uri).map(|path| paths::normalize(&path))
}

// JSON: the result of the protocol's `initialize` request.
pub fn server_capabilities(initialize_result: &Value) -> ServerCapabilities {
    let caps = initialize_result.get("capabilities");
    let flag = |name: &str| {
        caps.and_then(|caps| caps.get(name))
            .is_some_and(|value| !matches!(value, Value::Null | Value::Bool(false)))
    };
    let save = caps
        .and_then(|caps| caps.get("textDocumentSync"))
        .and_then(|sync| sync.get("save"))
        .and_then(save_mode);
    ServerCapabilities {
        save,
        hover: flag("hoverProvider"),
        definition: flag("definitionProvider"),
        references: flag("referencesProvider"),
        document_symbol: flag("documentSymbolProvider"),
        workspace_symbol: flag("workspaceSymbolProvider"),
    }
}

// JSON: the protocol's `TextDocumentSyncOptions.save`, a boolean or
// `SaveOptions`.
fn save_mode(save: &Value) -> Option<SaveMode> {
    match save {
        Value::Bool(true) => Some(SaveMode::Plain),
        Value::Object(options) => {
            if options
                .get("includeText")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                Some(SaveMode::WithText)
            } else {
                Some(SaveMode::Plain)
            }
        },
        _ => None,
    }
}

// JSON: the params of the protocol's `textDocument/publishDiagnostics`
// notification.
pub fn publish_diagnostics(params: &Value) -> Option<PublishedDiagnostics> {
    let path = path_of(params.get("uri")?.as_str()?)?;
    let version = params.get("version").and_then(Value::as_i64);
    let diagnostics = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| diagnostic(item, &path))
                .collect()
        })
        .unwrap_or_default();
    Some(PublishedDiagnostics {
        path,
        version,
        diagnostics,
    })
}

// JSON: the protocol's `Diagnostic`.
fn diagnostic(value: &Value, path: &Path) -> Option<Diagnostic> {
    Some(Diagnostic {
        path: path.to_path_buf(),
        range: range(value.get("range")?)?,
        severity: Severity::from_lsp(value.get("severity").and_then(Value::as_u64)),
        message: value.get("message")?.as_str()?.to_owned(),
        source: value
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_owned),
        code: value.get("code").and_then(code_text),
    })
}

// JSON: the protocol's diagnostic code, an integer or a string.
fn code_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

// JSON: a protocol `uinteger` field.
fn number(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
}

// JSON: the protocol's `Position`.
pub fn position(value: &Value) -> Option<Position> {
    Some(Position {
        line: number(value.get("line"))?,
        character: number(value.get("character"))?,
    })
}

// JSON: the protocol's `Range`.
pub fn range(value: &Value) -> Option<Range> {
    Some(Range {
        start: position(value.get("start")?)?,
        end: position(value.get("end")?)?,
    })
}

// JSON: a definition or references result: `Location`, `Location[]` or
// `LocationLink[]`.
pub fn locations(value: &Value) -> Vec<Location> {
    match value {
        Value::Array(items) => items.iter().filter_map(location).collect(),
        Value::Object(_) => location(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

// JSON: the protocol's `Location` or `LocationLink`.
fn location(value: &Value) -> Option<Location> {
    if let Some(target) = value.get("targetUri").and_then(Value::as_str) {
        let span = value
            .get("targetSelectionRange")
            .and_then(range)
            .or_else(|| value.get("targetRange").and_then(range))?;
        return Some(Location {
            path: path_of(target)?,
            range: span,
        });
    }
    Some(Location {
        path: path_of(value.get("uri")?.as_str()?)?,
        range: range(value.get("range")?)?,
    })
}

// JSON: the result of the protocol's `textDocument/hover` request.
pub fn hover(value: &Value) -> Option<Hover> {
    let contents = hover_text(value.get("contents")?);
    if contents.trim().is_empty() {
        return None;
    }
    Some(Hover {
        contents,
        range: value.get("range").and_then(range),
    })
}

// JSON: hover contents: `MarkupContent`, `MarkedString` or `MarkedString[]`.
fn hover_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(hover_text)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Value::Object(map) => {
            let text = map.get("value").and_then(Value::as_str).unwrap_or_default();
            match map.get("language").and_then(Value::as_str) {
                Some(language) if !text.is_empty() => format!("```{language}\n{text}\n```"),
                _ => text.to_owned(),
            }
        },
        _ => String::new(),
    }
}

// JSON: the configured server settings and the section a
// `workspace/configuration` item asks for.
pub fn configuration_section(settings: Option<&Value>, section: Option<&str>) -> Value {
    let Some(section) = section.filter(|section| !section.is_empty()) else {
        return settings.cloned().unwrap_or(Value::Null);
    };
    let mut current = settings;
    for key in section.split('.') {
        current = current.and_then(|value| value.get(key));
    }
    current.cloned().unwrap_or(Value::Null)
}
