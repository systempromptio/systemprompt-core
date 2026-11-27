//! Symbol queries fanned out to every server for a file, with their answers
//! merged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use futures_util::future::join_all;
use serde_json::{Value, json};
use tracing::debug;

use super::LspManager;
use crate::client::{Capability, Client};
use crate::error::{LspError, chain};
use crate::types::{Hover, Location, Position, Symbol};
use crate::{parse, uri};

impl LspManager {
    pub async fn hover(&self, path: &Path, position: Position) -> Result<Option<Hover>, LspError> {
        let results = self
            .query(path, Capability::Hover, "textDocument/hover", |uri| {
                position_params(uri, position)
            })
            .await?;
        let mut hovers: Vec<Hover> = results.iter().filter_map(parse::hover).collect();
        if hovers.len() <= 1 {
            return Ok(hovers.pop());
        }
        let range = hovers.iter().find_map(|hover| hover.range);
        let contents = hovers
            .into_iter()
            .map(|hover| hover.contents)
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        Ok(Some(Hover { contents, range }))
    }

    pub async fn definition(
        &self,
        path: &Path,
        position: Position,
    ) -> Result<Vec<Location>, LspError> {
        let results = self
            .query(
                path,
                Capability::Definition,
                "textDocument/definition",
                |uri| position_params(uri, position),
            )
            .await?;
        Ok(merge_locations(&results))
    }

    pub async fn references(
        &self,
        path: &Path,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, LspError> {
        let results = self
            .query(
                path,
                Capability::References,
                "textDocument/references",
                |uri| {
                    let mut params = position_params(uri, position);
                    if let Some(map) = params.as_object_mut() {
                        map.insert(
                            "context".to_owned(),
                            json!({ "includeDeclaration": include_declaration }),
                        );
                    }
                    params
                },
            )
            .await?;
        Ok(merge_locations(&results))
    }

    pub async fn document_symbols(&self, path: &Path) -> Result<Vec<Symbol>, LspError> {
        let resolved = self.resolve(path);
        let results = self
            .query(
                path,
                Capability::DocumentSymbol,
                "textDocument/documentSymbol",
                |uri| json!({ "textDocument": { "uri": uri } }),
            )
            .await?;
        Ok(results
            .iter()
            .flat_map(|result| parse::document_symbols(result, &resolved))
            .collect())
    }

    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<Symbol>, LspError> {
        let clients: Vec<Arc<Client>> = self
            .running_clients()
            .into_iter()
            .filter(|client| client.supports(Capability::WorkspaceSymbol))
            .collect();
        if clients.is_empty() {
            return Err(LspError::NoRunningServer);
        }
        let params = json!({ "query": query });
        let requests = clients
            .iter()
            .map(|client| client.request("workspace/symbol", params.clone()));
        let values = collect_results(join_all(requests).await)?;
        let mut symbols: Vec<Symbol> = values.iter().flat_map(parse::workspace_symbols).collect();
        symbols.dedup_by(|a, b| a.name == b.name && a.path == b.path && a.range == b.range);
        Ok(symbols)
    }

    // JSON: protocol request params per document URI in, raw protocol results out.
    async fn query(
        &self,
        path: &Path,
        capability: Capability,
        method: &'static str,
        params: impl Fn(&str) -> Value,
    ) -> Result<Vec<Value>, LspError> {
        let path = self.resolve(path);
        self.touch(&path, false).await?;
        let clients: Vec<Arc<Client>> = self
            .clients_for(&path)
            .await?
            .into_iter()
            .filter(|client| client.supports(capability))
            .collect();
        if clients.is_empty() {
            return Err(LspError::Unsupported {
                operation: method,
                path,
            });
        }
        let params = params(&uri::path_to_uri(&path));
        let requests = clients
            .iter()
            .map(|client| client.request(method, params.clone()));
        collect_results(join_all(requests).await)
    }
}

// JSON: the protocol's `TextDocumentPositionParams`.
fn position_params(uri: &str, position: Position) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": position.line, "character": position.character }
    })
}

// JSON: raw protocol results from each server asked.
fn collect_results(results: Vec<Result<Value, LspError>>) -> Result<Vec<Value>, LspError> {
    let mut values = Vec::new();
    let mut first_error = None;
    for result in results {
        match result {
            Ok(value) => values.push(value),
            Err(error) => {
                debug!(error = %chain(&error), "language server query failed");
                if first_error.is_none() {
                    first_error = Some(error);
                }
            },
        }
    }
    match first_error {
        Some(error) if values.is_empty() => Err(error),
        _ => Ok(values),
    }
}

// JSON: definition or references results, one per server.
fn merge_locations(results: &[Value]) -> Vec<Location> {
    let mut locations: Vec<Location> = results.iter().flat_map(parse::locations).collect();
    locations.sort();
    locations.dedup();
    locations
}
