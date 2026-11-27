//! Forward-compatible reading of a verified manifest payload.
//!
//! A newer instance may name a host this build does not know, and an entry may
//! carry an id this build's validators refuse. Neither stops the device
//! syncing: before the payload is decoded into
//! [`SignedManifest`](systemprompt_models::bridge::manifest::SignedManifest),
//! an unknown host id (in `enabled_hosts`, and as a key of
//! `host_model_protocols` and `host_extensions`), a malformed extension id, a
//! plugin with a malformed id, and a managed MCP server with a malformed id,
//! name or URL are dropped and logged at warn. Everything else is decoded as
//! the instance signed it, and still fails the manifest when malformed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use systemprompt_identifiers::{McpServerId, PluginId, ValidatedUrl};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::ids::HostExtensionId;

const ENABLED_HOSTS: &str = "enabled_hosts";
const HOST_MODEL_PROTOCOLS: &str = "host_model_protocols";
const HOST_EXTENSIONS: &str = "host_extensions";
const PLUGINS: &str = "plugins";
const MANAGED_MCP_SERVERS: &str = "managed_mcp_servers";

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
pub(super) fn drop_unreadable(body: &mut Value) {
    let Some(body) = body.as_object_mut() else {
        return;
    };
    if let Some(Value::Array(hosts)) = body.get_mut(ENABLED_HOSTS) {
        keep_readable::<HostKind>(hosts, ENABLED_HOSTS);
    }
    for field in [HOST_MODEL_PROTOCOLS, HOST_EXTENSIONS] {
        if let Some(Value::Object(by_host)) = body.get_mut(field) {
            keep_known_hosts(by_host, field);
        }
    }
    if let Some(Value::Object(by_host)) = body.get_mut(HOST_EXTENSIONS) {
        for extensions in by_host.values_mut().filter_map(Value::as_array_mut) {
            keep_readable::<HostExtensionId>(extensions, HOST_EXTENSIONS);
        }
    }
    if let Some(Value::Array(plugins)) = body.get_mut(PLUGINS) {
        keep_entries(plugins, PLUGINS, |plugin| {
            field_reads::<PluginId>(plugin, "id")
        });
    }
    if let Some(Value::Array(servers)) = body.get_mut(MANAGED_MCP_SERVERS) {
        keep_entries(servers, MANAGED_MCP_SERVERS, |server| {
            field_reads::<Option<McpServerId>>(server, "id")
                && field_reads::<McpServerId>(server, "name")
                && field_reads::<ValidatedUrl>(server, "url")
        });
    }
}

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
fn reads<T: DeserializeOwned>(value: &Value) -> bool {
    T::deserialize(value).is_ok()
}

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
fn field_reads<T: DeserializeOwned>(entry: &Value, field: &str) -> bool {
    entry.get(field).is_none_or(reads::<T>)
}

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
fn keep_readable<T: DeserializeOwned>(values: &mut Vec<Value>, field: &'static str) {
    values.retain(|value| {
        let readable = reads::<T>(value);
        if !readable {
            tracing::warn!(field, value = %value, "manifest value not understood by this build; skipped");
        }
        readable
    });
}

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
fn keep_known_hosts(by_host: &mut Map<String, Value>, field: &'static str) {
    by_host.retain(|host, _| {
        let known = host.parse::<HostKind>().is_ok();
        if !known {
            tracing::warn!(field, host = %host, "manifest names a host unknown to this build; skipped");
        }
        known
    });
}

// JSON: the verified payload is edited as a JSON tree before core's manifest type decodes it.
fn keep_entries(entries: &mut Vec<Value>, field: &'static str, readable: impl Fn(&Value) -> bool) {
    entries.retain(|entry| {
        let keep = readable(entry);
        if !keep {
            let id = entry.get("id").or_else(|| entry.get("name"));
            tracing::warn!(field, id = ?id, "manifest entry carries a malformed id; skipped");
        }
        keep
    });
}
