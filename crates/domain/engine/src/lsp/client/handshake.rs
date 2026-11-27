//! The `initialize` exchange: what the client tells a server about itself and
//! its workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::{Client, ClientSettings};
use crate::lsp::error::LspError;
use crate::lsp::parse::{self, ServerCapabilities};
use crate::lsp::spec::ServerSpec;
use crate::lsp::uri;

// JSON: the protocol's `WorkspaceFolder`.
pub(super) fn workspace_folder(root: &Path) -> Value {
    let name = root
        .file_name()
        .map_or_else(|| root.to_string_lossy(), |name| name.to_string_lossy());
    json!({ "uri": uri::path_to_uri(root), "name": name })
}

// JSON: the protocol's `ClientCapabilities`.
fn client_capabilities() -> Value {
    let symbol_kinds: Vec<u32> = (1..=26).collect();
    json!({
        "general": { "positionEncodings": ["utf-16"] },
        "window": { "workDoneProgress": true },
        "workspace": {
            "configuration": true,
            "workspaceFolders": true,
            "didChangeConfiguration": { "dynamicRegistration": false },
            "symbol": { "dynamicRegistration": false, "symbolKind": { "valueSet": symbol_kinds } }
        },
        "textDocument": {
            "synchronization": {
                "dynamicRegistration": false,
                "didSave": true,
                "willSave": false,
                "willSaveWaitUntil": false
            },
            "publishDiagnostics": {
                "relatedInformation": false,
                "versionSupport": true,
                "codeDescriptionSupport": true,
                "tagSupport": { "valueSet": [1, 2] }
            },
            "hover": { "dynamicRegistration": false, "contentFormat": ["markdown", "plaintext"] },
            "definition": { "dynamicRegistration": false, "linkSupport": true },
            "references": { "dynamicRegistration": false },
            "documentSymbol": {
                "dynamicRegistration": false,
                "hierarchicalDocumentSymbolSupport": true,
                "symbolKind": { "valueSet": symbol_kinds }
            }
        }
    })
}

// JSON: the params of the protocol's `initialize` request.
fn initialize_params(root: &Path, spec: &ServerSpec, settings: &ClientSettings) -> Value {
    let mut params = Map::new();
    params.insert("processId".to_owned(), Value::from(std::process::id()));
    params.insert(
        "clientInfo".to_owned(),
        json!({ "name": settings.name, "version": settings.version }),
    );
    params.insert("rootUri".to_owned(), Value::from(uri::path_to_uri(root)));
    params.insert(
        "rootPath".to_owned(),
        Value::from(root.to_string_lossy().into_owned()),
    );
    params.insert(
        "workspaceFolders".to_owned(),
        json!([workspace_folder(root)]),
    );
    params.insert("capabilities".to_owned(), client_capabilities());
    params.insert("trace".to_owned(), Value::from("off"));
    if let Some(options) = &spec.initialization_options {
        params.insert("initializationOptions".to_owned(), options.clone());
    }
    Value::Object(params)
}

impl Client {
    pub(super) async fn initialize(
        &self,
        spec: &ServerSpec,
        settings: &ClientSettings,
    ) -> Result<ServerCapabilities, LspError> {
        let params = initialize_params(&self.shared.root, spec, settings);
        let result = self
            .shared
            .request("initialize", params, self.timeouts.initialize)
            .await?;
        let capabilities = parse::server_capabilities(&result);
        self.shared.notify("initialized", json!({}))?;
        if let Some(settings) = &spec.initialization_options {
            self.shared.notify(
                "workspace/didChangeConfiguration",
                json!({ "settings": settings }),
            )?;
        }
        Ok(capabilities)
    }
}
