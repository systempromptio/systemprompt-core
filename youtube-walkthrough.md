# YouTube — systemprompt.io Core + Cowork Walkthrough

## Title (recommended)

Inside the systemprompt.io Binary — A Rust Walkthrough of Core + Cowork Governance

### Alternates

- One Binary, Full AI Governance — Technical Tour of systemprompt-core and Cowork
- How systemprompt.io Governs an AI Fleet from a 50MB Rust Binary (Code Walkthrough)

## Description

A technical walkthrough of systemprompt.io — the self-hosted binary that governs inference, auditing, evals, and every tool call across your AI fleet. One 50MB Rust binary. PostgreSQL as the only dependency. Your infrastructure, your source code, your data plane.

In this video I open up the codebase and show how it actually works:

- Crate architecture — Shared → Infra → Domain → App → Entry → Facade, and why dependencies only flow downward
- Handler-boundary enforcement — the four sub-millisecond checks (permissions, secret scanning, blocklists, rate limits) that run before any tool executes
- Identity-bound audit trails — every action traced to an authenticated user, structured JSON end-to-end
- Provider-agnostic execution — Claude, OpenAI, Gemini, Groq, or self-hosted models behind one interface
- Typed identifiers, repository pattern, and compile-time-checked SQLx
- Extension framework — compile-time registration via `inventory`
- A2A and MCP protocol surfaces inside the domain layer
- Claude Cowork — the binary that runs Cowork on customer infrastructure, IPC, proxy probe, per-agent state, and sync back to core

### Chapters

```
00:00  What systemprompt.io actually is
01:30  Repo tour and dependency flow
05:00  Shared: models, traits, typed identifiers
09:00  Infra: database, events, security, config
14:00  Domain: agent, mcp, ai — and the governance boundary
20:00  App + Entry: runtime, API, CLI
24:00  Extension framework
30:00  Cowork binary architecture
36:00  Running an end-to-end governed tool call
42:00  Compliance posture and what's next
```

### Links

- Site: https://systemprompt.io
- Repo: https://github.com/<your-org>/systemprompt-core

### Tags

#rust #aigovernance #mcp #a2a #selfhosted #claudecode
