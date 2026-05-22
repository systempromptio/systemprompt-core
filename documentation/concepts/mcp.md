# MCP integration

How systemprompt-core integrates the Model Context Protocol (MCP): server lifecycle, the central registry, the streamable-HTTP transport, and Ed25519-signed bridge manifests.

MCP (Model Context Protocol) is how the platform exposes tools and resources to agents. The implementation lives in `crates/domain/mcp` and is built on `rmcp` 1.6. The platform treats governance as the MCP transport layer: tool calls cross the MCP boundary, where they are recorded and governed, rather than executing as opaque in-process calls.

## MCP servers and their lifecycle

An MCP server provides tools and resources. The platform manages servers as processes through a lifecycle and monitoring layer (`crates/domain/mcp/src/services/{lifecycle,process,monitoring}`). Servers are spawned as subprocesses with configuration and secrets passed explicitly through environment variables — there is no fuzzy profile discovery inside a subprocess, and the JWT secret must be identical across parent and child for token validation to hold.

A process monitor reconciles database-recorded service PIDs against live processes on a periodic loop and marks crashes, so a server that dies is observed rather than silently absent. The MCP orchestrator daemon handles its own shutdown signal, draining cleanly on stop.

## The registry

Rather than each consumer holding its own MCP server configuration, the platform keeps a single central registry (`RegistryService`, constructed in the runtime bootstrap with the system-admin identity). The registry is the source of truth for which servers exist and how to reach them, which avoids local configuration drift across consumers. It is exposed at `/api/v1/mcp/registry`.

## The streamable-HTTP transport

MCP servers are reached over a streamable-HTTP transport, mounted per server at:

```
/api/v1/mcp/{server}/mcp
```

A request names the target server in the path; the platform resolves it through the registry and proxies to the running server process. RBAC is enforced on this path — and notably the MCP RBAC layer does validate the token audience, unlike the primary API extractor (see [authentication.md](authentication.md)).

Tool executions are logged at the server, not the client. The MCP client in core deliberately does no execution logging; the server records each call (input, output, structured content, timing, status) to `mcp_tool_executions`. This keeps a single source of truth with the complete payload, and links each execution back to the originating AI tool call through shared identifiers (`ai_tool_call_id`, `mcp_execution_id`) carried in the request context.

## Signed bridge manifests

The platform signs bridge manifests so a downstream consumer can verify a manifest's integrity and origin. Signing uses **Ed25519** (`crates/infra/security/src/manifest_signing.rs`), not a shared-secret MAC.

```
manifest value
   │
   ▼
JSON Canonicalization Scheme (RFC 8785)   ── semantically-equal payloads
   │                                          canonicalise identically
   ▼
Ed25519 sign  (key derived from a 32-byte seed loaded by SecretsBootstrap)
   │
   ▼
base64 signature  +  base64 public key (served at /api/v1/.../bridge/pubkey)
```

The signing key is derived from a 32-byte seed loaded through `SecretsBootstrap` and cached in a process-wide `OnceLock`, so key derivation runs at most once per process. Manifests are canonicalised with the JSON Canonicalization Scheme (RFC 8785) before signing, so two semantically-equivalent payloads produce identical signatures and verification is stable across serialisation differences. The public key is published so a verifier needs no shared secret — this is asymmetric signing, with verification using only the public key.

> The bridge-manifest HTTP handlers currently perform blocking `std::fs` I/O. This is a known reliability item on an async server, noted here for completeness.

## How agents use MCP

The MCP domain depends only on infra and shared crates; it has no edge to the agent or AI domains. Agents reach tools through the runtime/entry wiring rather than a direct crate dependency (see [architecture.md](architecture.md)). When an agent decides to call a tool, the AI request is logged to `ai_requests`, the tool call to `ai_request_tool_calls`, and the MCP execution to `mcp_tool_executions` — the three rows linked by shared identifiers so the full path is reconstructable.

## See also

- [a2a-protocol.md](a2a-protocol.md) — the agents that consume MCP tools.
- [authentication.md](authentication.md) — per-server OAuth2 and the MCP RBAC audience check.
- [The threat model](../security/threat-model.md) — the manifest-signing trust boundary in context.
