//! Reading document and workspace symbol results, flattening the document
//! symbol tree in walk order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::Value;

use super::{number, path_of, range};
use crate::types::{Symbol, SymbolKind};

// JSON: the result of `textDocument/documentSymbol`: `DocumentSymbol[]` or
// `SymbolInformation[]`.
pub fn document_symbols(value: &Value, path: &Path) -> Vec<Symbol> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        if item.get("location").is_some() {
            out.extend(symbol_information(item));
        } else {
            let place = SymbolPlace {
                path,
                container: None,
                depth: 0,
            };
            document_symbol(item, &place, &mut out);
        }
    }
    out
}

// JSON: a protocol symbol's `kind` field.
fn kind(value: &Value) -> Option<SymbolKind> {
    number(value.get("kind")).map(SymbolKind)
}

struct SymbolPlace<'a> {
    path: &'a Path,
    container: Option<&'a str>,
    depth: u32,
}

// JSON: the protocol's `DocumentSymbol`, walked with its children.
fn document_symbol(value: &Value, place: &SymbolPlace<'_>, out: &mut Vec<Symbol>) {
    let Some(name) = value.get("name").and_then(Value::as_str) else {
        return;
    };
    let Some(kind) = kind(value) else {
        return;
    };
    let span = value
        .get("selectionRange")
        .and_then(range)
        .or_else(|| value.get("range").and_then(range));
    let Some(span) = span else {
        return;
    };
    out.push(Symbol {
        name: name.to_owned(),
        kind,
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .filter(|detail| !detail.is_empty())
            .map(str::to_owned),
        container: place.container.map(str::to_owned),
        path: place.path.to_path_buf(),
        range: span,
        depth: place.depth,
    });
    let child_place = SymbolPlace {
        path: place.path,
        container: Some(name),
        depth: place.depth.saturating_add(1),
    };
    for child in value
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        document_symbol(child, &child_place, out);
    }
}

// JSON: the protocol's `SymbolInformation`.
fn symbol_information(value: &Value) -> Option<Symbol> {
    let location = value.get("location")?;
    Some(Symbol {
        name: value.get("name")?.as_str()?.to_owned(),
        kind: kind(value)?,
        detail: None,
        container: value
            .get("containerName")
            .and_then(Value::as_str)
            .filter(|container| !container.is_empty())
            .map(str::to_owned),
        path: path_of(location.get("uri")?.as_str()?)?,
        range: location.get("range").and_then(range).unwrap_or_default(),
        depth: 0,
    })
}

// JSON: the result of the protocol's `workspace/symbol` request.
pub fn workspace_symbols(value: &Value) -> Vec<Symbol> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(symbol_information).collect())
        .unwrap_or_default()
}
