# The provider-facing gateway

How systemprompt-core proxies model traffic to upstream providers: the gateway endpoints, request routing, and the controls — quota, policy, safety screening, and audit — applied to every proxied call.

The gateway is the provider-facing proxy. It accepts model requests on a stable, provider-shaped surface, screens and meters them, routes them to a configured upstream provider, and records a full audit trail. It is implemented in `crates/entry/api/src/routes/gateway` and `crates/entry/api/src/services/gateway`.

Note the distinction from the internal model path: the gateway proxy is a self-contained subsystem with its own outbound HTTP adapters. It shares canonical wire types and codecs with the internal AI service in `crates/domain/ai`, but does not inherit that path's `ResilientProvider` policy (see [The resilience boundary](#resilience)).

## The gateway surface

The gateway mounts under the base path `/v1`:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/messages` | POST | Anthropic-shaped messages request (inbound-adapted) |
| `/v1/responses` | POST | OpenAI-responses-shaped request (inbound-adapted) |
| `/v1/chat/completions` | POST | OpenAI Chat Completions request |
| `/v1/models` | GET | List available models from the catalog |
| `/v1/otel` (and `/v1/otel/{*rest}`) | POST | OTLP ingest of traces/logs/metrics from clients |

The inference endpoints accept provider request shapes through inbound adapters and converge on the same internal handler, so a caller can speak the request dialect it already knows. Every gateway request passes through an access-logging middleware that records method, path, status, and elapsed time both to `tracing` and to the `logs` table.

Request and response schemas for these endpoints belong in the reference material.

## Routing and the catalog

A gateway request names a model; the gateway resolves it to a configured route and dispatches the call. Routes live in the services tree (`services/ai/gateway.yaml`, the `gateway:` key) and name providers declared in the services provider registry (`services/ai/providers.yaml`, the `providers:` key); both are loaded once at boot into `ServicesBootstrap` and resolved by the gateway registry (`crates/entry/api/src/services/gateway/registry.rs`). Each route references a provider whose wire protocol is Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or Gemini — handled by the matching outbound adapter under `crates/entry/api/src/services/gateway/protocol/outbound/`. The `/v1/models` endpoint surfaces the catalog of models the configured routes expose. A request that names an unconfigured model is rejected rather than dispatched.

Every inbound body is parsed into one provider-neutral request (`systemprompt_wire::canonical::CanonicalRequest`) before it is governed, and the outbound adapter renders that request in the provider's dialect. Two lanes leave the gateway. When the client's wire matches the provider's and no system-prompt override applies, the adapter forwards the client's own bytes (model name and output cap normalised, caller identity stripped), so beta-gated fields the canonical model does not describe survive and the audited `prepared_body_sha256` equals `request_body_sha256`. Otherwise the body is rebuilt from the canonical request. The canonical model carries Anthropic's prompt-cache breakpoints — `cache_control` on system blocks, content blocks and tools — so a rebuild puts every breakpoint back where the client set it; a system-prompt override replaces the system blocks and keeps the conversation and tool breakpoints. Wires without prompt caching ignore the field.

## What the gateway enforces

Each proxied request passes through a fixed sequence of gateway-owned controls before and after the upstream call:

| Control | Behaviour | Source |
|---------|-----------|--------|
| Quota | Subject-keyed usage windows; enforcing policies reject exhausted quotas. Cost is accounted after completion, so concurrent requests can exceed a ceiling. | `services/gateway/quota.rs` |
| Policy | Request admissibility checks against the configured gateway policy. | `services/gateway/policy.rs` |
| Safety | Heuristic content screening on the request. | `services/gateway/safety/` |
| Audit | Every request and the streamed/whole response are recorded (method, path, status, latency, token counts, pricing). | `services/gateway/audit/`, `stream_tap/`, `pricing.rs` |
| SSRF guard | Outbound route endpoints are validated by the shared `validate_outbound_url` guard. | `crates/shared/models/src/net/mod.rs` |

## Safety scanners

A gateway policy's `safety` block names the scanners that judge each request (and, when `block_response_categories` is set, each response). Core ships two: `heuristic` (phrase list, email and card-number detection) and `null`. Any other scanner — a vendor guardrail service, an in-house classifier — is implemented outside core and registered at compile time with `register_safety_scanner!`.

Every scanner is governed the same way, whoever wrote it:

```yaml
policies:
  - name: default
    spec:
      safety:
        scanners: [heuristic, vendor_guard]
        block_categories: [jailbreak, prompt_injection]
        scanner_settings:
          vendor_guard:
            fail_mode: open        # open | closed (default closed)
            timeout_ms: 1500       # default 5000, at least 1
            config:                # opaque to core, handed to the scanner
              endpoint: https://guard.example.com/v1/screen
              template: strict
              credential_secret: vendor_guard_key
```

- `timeout_ms` bounds the scan in the request, history and response phases. A scan that overruns it fails with `ScanError::TimedOut`.
- A failed or timed-out scan is persisted as a `scanner_failure` finding. Under `fail_mode: closed` it blocks the request; under `open` it is recorded with `blocked = false` and the request proceeds. `safety.mode: warn` never blocks.
- `config` is not interpreted by core. Its keys are the scanner's own.
- Validation refuses `scanner_settings` for a scanner not listed in `scanners`, and a `timeout_ms` of 0.

### Writing an external scanner

An extension crate depends on `systemprompt-gateway`, implements `SafetyScanner`, and registers a factory that receives the scanner's `ScannerSettings`:

```rust
use std::sync::Arc;

use systemprompt_gateway::protocol::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_gateway::{
    Finding, PHASE_REQUEST, SafetyScanner, ScanError, ScannerSettings, Severity,
    register_safety_scanner,
};

struct VendorGuard {
    endpoint: Option<String>,
    template: String,
}

impl VendorGuard {
    fn from_settings(settings: &ScannerSettings) -> Self {
        let text = |key: &str| settings.config.get(key).and_then(|v| v.as_str()).map(str::to_owned);
        Self {
            endpoint: text("endpoint"),
            template: text("template").unwrap_or_else(|| "default".to_owned()),
        }
    }
}

#[async_trait::async_trait]
impl SafetyScanner for VendorGuard {
    fn name(&self) -> &'static str {
        "vendor_guard"
    }

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        let Some(endpoint) = &self.endpoint else {
            return Err(ScanError::Failed {
                scanner: "vendor_guard",
                reason: "config.endpoint is not set".to_owned(),
            });
        };
        for (_part, text) in req.safety_parts(false) {
            // call `endpoint` with `self.template` and `text`; map its verdicts to findings
        }
        Ok(Vec::new())
    }

    async fn scan_response_final(&self, _r: &CanonicalResponse) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}

register_safety_scanner!(VendorGuard::from_settings, name = "vendor_guard");
```

The factory runs once per policy evaluation with that policy's settings (defaults when the policy has no `scanner_settings` entry for the scanner). The scanner does not need its own timeout or fail-open logic: the gateway applies `timeout_ms` and `fail_mode` around every call. A registration that reuses a built-in name (`heuristic`, `null`) is rejected.

## Resilience

The gateway uses a shared outbound HTTP client and a bounded retry policy. Transient
HTTP 429 and 503 responses allow up to four total attempts before response bytes are
forwarded. Backoff starts at one second, caps its base delay at 30 seconds, adds jitter
and honors longer `retry-after` values. Transport failures and other statuses follow their
error paths. See
`crates/entry/api/src/services/gateway/protocol/outbound/retry.rs`.

The gateway does not inherit the internal AI service’s circuit-breaker and bulkhead
wrapper. Configure deployment-level concurrency and request-duration limits for the
expected streaming workload. Response-body failures after headers are sent are reported
through protocol-specific stream errors and terminal audit records.


## See also

- [a2a-protocol.md](a2a-protocol.md) — agents are the primary internal consumers of the gateway.
- [architecture.md](architecture.md) — where the AI domain and the entry-layer gateway routes sit in the layering.
- [The stability contract](../security/stability-contract.md) for the surface's compatibility guarantees.
