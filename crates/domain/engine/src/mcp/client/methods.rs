//! The MCP methods this client calls: tools, resources and prompts, listed
//! across pages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::McpClient;
use super::options::RequestOptions;
use crate::error::McpError;
use crate::jsonrpc::METHOD_NOT_FOUND;
use crate::types::{CallResult, Prompt, PromptResult, Resource, ResourceContent, ToolInfo};

const MAX_PAGES: usize = 64;

impl McpClient {
    pub async fn ping(&self, options: RequestOptions) -> Result<(), McpError> {
        self.request("ping", None, options).await.map(|_| ())
    }

    pub async fn list_tools(&self, options: RequestOptions) -> Result<Vec<ToolInfo>, McpError> {
        self.paginate("tools/list", "tools", options).await
    }

    // JSON: tool arguments are the object the caller built for the tool's input
    // schema.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        options: RequestOptions,
    ) -> Result<CallResult, McpError> {
        let arguments = if arguments.is_null() {
            Value::Object(Map::new())
        } else {
            arguments
        };
        let params = json!({"name": name, "arguments": arguments});
        let result = self.request("tools/call", Some(params), options).await?;
        Ok(CallResult::from_value(&result))
    }

    pub async fn list_resources(&self, options: RequestOptions) -> Result<Vec<Resource>, McpError> {
        self.paginate("resources/list", "resources", options).await
    }

    pub async fn read_resource(
        &self,
        uri: &str,
        options: RequestOptions,
    ) -> Result<Vec<ResourceContent>, McpError> {
        let result = self
            .request("resources/read", Some(json!({"uri": uri})), options)
            .await?;
        Ok(parse_items(&result, "contents", "resources/read"))
    }

    pub async fn list_prompts(&self, options: RequestOptions) -> Result<Vec<Prompt>, McpError> {
        self.paginate("prompts/list", "prompts", options).await
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: &BTreeMap<String, String>,
        options: RequestOptions,
    ) -> Result<PromptResult, McpError> {
        let params = json!({"name": name, "arguments": arguments});
        let result = self.request("prompts/get", Some(params), options).await?;
        Ok(PromptResult::from_value(&result))
    }

    async fn paginate<T: DeserializeOwned>(
        &self,
        method: &str,
        key: &str,
        options: RequestOptions,
    ) -> Result<Vec<T>, McpError> {
        let RequestOptions { timeout, cancel } = options;
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen = HashSet::new();
        for _ in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|cursor| json!({"cursor": cursor}));
            let page_options = RequestOptions {
                timeout,
                cancel: cancel.clone(),
            };
            let page = match self.request(method, params, page_options).await {
                Ok(page) => page,
                Err(McpError::Rpc {
                    code: METHOD_NOT_FOUND,
                    ..
                }) if cursor.is_none() => {
                    return Ok(Vec::new());
                },
                Err(error) => return Err(error),
            };
            items.extend(parse_items::<T>(&page, key, method));
            match page.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() && seen.insert(next.to_owned()) => {
                    cursor = Some(next.to_owned());
                },
                _ => return Ok(items),
            }
        }
        tracing::debug!(method, pages = MAX_PAGES, "stopped following pages");
        Ok(items)
    }
}

// JSON: one page of a list result; each entry is read on its own and a
// malformed one is skipped.
fn parse_items<T: DeserializeOwned>(page: &Value, key: &str, method: &str) -> Vec<T> {
    let Some(entries) = page.get(key).and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match serde_json::from_value::<T>(entry.clone()) {
            Ok(item) => Some(item),
            Err(error) => {
                tracing::debug!(method, error = %error, "skipping an entry the server listed");
                None
            },
        })
        .collect()
}
