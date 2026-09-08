//! Semantic state comparison excludes telemetry at explicit wire locations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use super::StatePayload;

impl StatePayload<'_> {
    pub fn semantic_value(mut self) -> Result<Value, serde_json::Error> {
        self.last_probe_at_unix = None;
        self.last_validation_at_unix = None;
        self.gateway_status.latency_ms = None;
        self.proxy_stats = super::payloads::ProxyStatsPayload::default();
        if let Some(token) = &mut self.cached_token {
            token.ttl_seconds = 0;
        }
        if let Some(identity) = &mut self.verified_identity {
            identity.verified_at_unix = 0;
            identity.exp_unix = None;
        }
        for host in &mut self.hosts.host_apps {
            if let Some(health) = &mut host.health {
                health.probed_at_unix = 0;
            }
        }
        let mut value = serde_json::to_value(self)?;
        if let Some(proxy) = value.get_mut("local_proxy").and_then(Value::as_object_mut) {
            proxy.remove("probed_at_unix");
            proxy.remove("latency_ms");
        }
        if let Some(servers) = value.get_mut("mcp_auth").and_then(Value::as_array_mut) {
            for server in servers {
                if let Some(server) = server.as_object_mut() {
                    server.remove("probed_at_unix");
                    server.remove("latency_ms");
                    server.remove("session_id");
                }
            }
        }
        Ok(value)
    }
}
