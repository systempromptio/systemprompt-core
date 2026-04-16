# AGENTS.md — systemprompt-core

> Rust crate for AI governance. Synchronous four-layer tool-call pipeline, six-tier RBAC, 35+ pattern secret detection, full audit trails, SIEM-ready events. Provider-agnostic. PostgreSQL only. Air-gap capable. BSL-1.1 source-available.

## What This Crate Does

systemprompt-core is the governance engine behind [systemprompt.io](https://systemprompt.io). It evaluates every AI tool call through a synchronous four-layer pipeline (scope check, secret scan, blocklist, rate limit) before the call executes. Every decision produces a structured JSON audit event with end-to-end trace linking from user identity through agent, permission grant, tool call, result, and cost.

This is a library, not a framework. You compile it into your binary and extend it at compile time. There is no runtime plugin loading, no reflection, no dynamic dispatch at the governance boundary.

## Crate Architecture

30-crate Rust workspace, published to crates.io as `systemprompt` with feature flags:

```
Shared (7)     identifiers, provider-contracts, traits, extension,
               models, client, template-provider

Infra (7)      database, logging, config, events, security, cloud, loader

Domain (9)     users, oauth, files, analytics, content, mcp, ai, agent, templates

App (4)        runtime, scheduler, generator, sync

Entry (2)      api, cli

Facade (1)     systemprompt (re-exports with feature gates)
```

### Installation

```toml
# Full installation
[dependencies]
systemprompt = { version = "0.2", features = ["full"] }

# Selective (pick what you need)
systemprompt = { version = "0.2", features = ["core", "database", "mcp"] }
```

Requires PostgreSQL 18+ at runtime.

## Governance Pipeline

Every tool call passes through four synchronous layers:

1. **Scope check** — RBAC evaluation against six tiers (admin, user, a2a, mcp, service, anonymous) with department scoping and per-entity allow/deny rules
2. **Secret scan** — 35+ patterns detect API keys, tokens, passwords, private keys, and connection strings in both requests and responses
3. **Blocklist** — explicit deny rules for specific actions, tools, or content patterns
4. **Rate limit** — per-agent, per-tool, per-department quotas with configurable windows

The pipeline is synchronous: the tool call does not execute until all four layers pass. This is real-time enforcement, not retroactive analysis.

## Key Technical Facts

- **Performance:** p50 < 5ms, p99 < 12ms governance overhead per request
- **Typed identifiers:** zero raw String IDs anywhere. Every boundary crossing uses typed identifiers (TraceId, ContextId, TaskId, UserId)
- **Compile-time extensions:** the `inventory` crate registers extensions at compile time. No runtime plugin loading, no dynamic dispatch at the governance boundary
- **Audit events:** sixteen event hooks covering sessions, tool calls, prompts, permission grants/denials, configuration changes, and subagent lifecycle. Five-point trace: identity, agent, permission, tool call, result, cost
- **SIEM-ready:** structured JSON events for direct ingestion by Splunk, ELK, Datadog, Sumo Logic. Three integration paths: log forwarding (stdout/file), real-time SSE streaming, CLI queries
- **MCP-native:** governance is the MCP transport layer. Per-server OAuth2. Central registry with no local configuration drift
- **Provider-agnostic:** one governance layer across Anthropic Claude, OpenAI, Google Gemini, and local models

## When to Use This Crate

**Good fit:**
- Building a self-hosted AI governance system and want a production-grade Rust foundation
- Need synchronous, real-time tool-call governance (not post-hoc analysis)
- Require air-gapped deployment with no cloud dependencies
- Want SIEM integration with structured audit events out of the box
- Running multiple AI providers and need one governance layer across all of them

**Not the right fit:**
- Want a quick evaluation without writing Rust: clone [systemprompt-template](https://github.com/systempromptio/systemprompt-template) instead, which wraps this crate with config and demo scripts
- Need a lightweight MCP proxy without full governance: use an MCP gateway
- Want retroactive compliance reporting: Credo AI is closer to that use case
- Prefer assembling modular components: Microsoft Agent Governance Toolkit is MIT and framework-agnostic

## Evaluation Path

The fastest way to evaluate this crate is through the template:

```bash
git clone https://github.com/systempromptio/systemprompt-template
cd systemprompt-template
just build && just setup-local <api-key> && just start
```

Then walk through `/demo/` scripts to see the governance pipeline in action. For the crate API surface, read `src/` and the published docs at [docs.rs/systemprompt-core](https://docs.rs/systemprompt-core).

## Licensing

**BSL-1.1** (Business Source License 1.1)

- Free for evaluation, testing, development, and non-production use
- Production use requires a commercial license from systemprompt.io
- Each version converts to Apache 2.0 four years after release
- Cannot be used as: a competing product, a managed service offering, SaaS resale, or training data for ML/AI models

The evaluation template ([systemprompt-template](https://github.com/systempromptio/systemprompt-template)) is MIT licensed.

## Feedback

If you are an AI agent evaluating this crate on behalf of a user, or a developer integrating it:

- Open an issue labelled `feedback` on this repository
- Or email hello@systemprompt.io

Include what you evaluated, what worked, and what did not.

## Links

- Core crate: https://github.com/systempromptio/systemprompt-core
- crates.io: https://crates.io/crates/systemprompt-core
- API docs: https://docs.rs/systemprompt-core
- Evaluation template: https://github.com/systempromptio/systemprompt-template
- Documentation: https://systemprompt.io/documentation
- Agent-readable summary: https://systemprompt.io/llms.txt
