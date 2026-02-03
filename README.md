# systemprompt.io

**Production infrastructure for AI agents. Self-hosted or cloud.**

AI infrastructure built for AI agents. Purpose-built in Rust for reliable orchestration, deep observability, and deterministic execution. Not another SDK—complete infrastructure with authentication, permissions, and multi-agent coordination on open standards (MCP, A2A, OAuth2).

**Playbooks** provide deterministic instruction rails that eliminate hallucination—your agents execute tested commands, not guessed syntax. When superintelligent systems manage your infrastructure, they need verified operational procedures, not prose.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://docs.rs/systemprompt/badge.svg)](https://docs.rs/systemprompt)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-purple.svg)](https://modelcontextprotocol.io/)
[![A2A](https://img.shields.io/badge/A2A-protocol-green.svg)](https://a2a-protocol.org/)
[![Discord](https://img.shields.io/badge/Discord-Join%20us-5865F2.svg)](https://discord.gg/wkAbSuPWpr)

[Discord](https://discord.gg/wkAbSuPWpr) · [Installation](https://systemprompt.io/documentation/installation) · [Playbooks](https://systemprompt.io/documentation/playbooks) · [Config](https://systemprompt.io/documentation/config) · [Services](https://systemprompt.io/documentation/services) · [Extensions](https://systemprompt.io/documentation/extensions) · [Crates.io](https://crates.io/crates/systemprompt)

**Questions or issues?** Join us on [Discord](https://discord.gg/wkAbSuPWpr) for help.

## Table of Contents

- [Why systemprompt.io?](#ship-agents-to-production-in-record-time)
- [Quick Start](#quick-start)
- [Using as a Library](#using-as-a-library)
- [Architecture](#architecture)
- [Extension Framework](#extension-framework)
- [License](#license)

## Ship agents to production in record time.

AI frameworks get you to a demo. Production requires everything else: auth, permissions, observability, multi-agent coordination. You shouldn't rebuild this infrastructure for every project.

systemprompt.io is the missing layer—a library you own, not a platform you rent:

- **Auth that works**: OAuth2/OIDC + WebAuthn, production-ready from day one
- **Permissions that scale**: Per-user, per-agent, per-tool scopes
- **Deployment that's real**: One command to cloud or self-host
- **Multi-agent that coordinates**: A2A protocol with shared state

50MB of production-ready Rust. You own the binary.

**Core capabilities:**
- **Complete Runtime**: Web API + agent processes + MCP servers with shared auth and database
- **Open Standards**: MCP, A2A, OAuth2, WebAuthn - no vendor lock-in
- **Agent-Executable CLI**: Your AI manages infrastructure directly via the same CLI you use
- **Native Rust**: Async-first on Tokio, zero-cost abstractions
- **Self-Hosted or Cloud**: Docker locally, or deploy to isolated VM with managed database
- **100% Extensible**: Build proprietary Rust extensions on the open core

### What You Get

A complete platform with built-in:
- **User Authentication**: OAuth2/OIDC, sessions, roles, and permissions
- **File Storage**: Upload, serve, and manage files with metadata
- **Content Management**: Markdown ingestion, search, and publishing
- **AI Integration**: Multi-provider LLM support with request logging
- **Analytics**: Session tracking, metrics, and usage reporting
- **Agent Orchestration**: A2A protocol for agent-to-agent communication
- **MCP Servers**: Tool and resource providers for AI clients

## Quick Start

```bash
# 1. Clone the template
git clone https://github.com/systempromptio/systemprompt-template my-project
cd my-project

# 2. Build
cargo build --release

# 3. Login
systemprompt cloud auth login

# 4. Create tenant
systemprompt cloud tenant create

# 5. Start
systemprompt infra services start --all
```

See the [systemprompt-template](https://github.com/systempromptio/systemprompt-template) for full installation instructions and configuration options.  

### Native MCP Client Support

Works out of the box with any MCP client - Claude Code, Claude Desktop, ChatGPT, and more. All transports are HTTP-native, supported by modern MCP clients.

```json
// claude_desktop_config.json
{
  "mcpServers": {
    "my-server": {
      "url": "https://my-tenant.systemprompt.io/api/v1/mcp/my-server/mcp",
      "transport": "streamable-http"
    }
  }
}
```

Your AI can now manage your entire infrastructure: deploy updates, query analytics, manage users, and orchestrate agents - all through natural conversation.

### Discovery API

Get agent and MCP connection details from the API at any time:

| Endpoint | Description |
|----------|-------------|
| `/.well-known/agent-card.json` | Default agent card |
| `/.well-known/agent-cards` | List all available agents |
| `/.well-known/agent-cards/{name}` | Specific agent card |
| `/api/v1/agents/registry` | Full agent registry with status |
| `/api/v1/mcp/registry` | All MCP servers with endpoints |

### Config as Code

Define your entire infrastructure in the `services/` directory - granular permissions for agents, MCP tools, and users backed by production-grade OAuth2 and WebAuthn:

```
services/
├── agents/           # Agent definitions with OAuth scopes
│   └── blog.yaml     # security: [oauth2: ["admin"]]
├── mcp/              # MCP servers with per-tool permissions
│   └── content.yaml  # oauth: { required: true, scopes: ["admin"] }
├── skills/           # Reusable agent capabilities
├── ai/               # Provider configs (Anthropic, OpenAI, Gemini)
├── content/          # Markdown content sources
├── scheduler/        # Cron jobs and background tasks
└── web/              # Theme, branding, navigation
```

**Granular Security:**
- **Agents**: OAuth2 scopes define who can interact with each agent
- **MCP Tools**: Per-tool OAuth requirements and audience restrictions
- **Users**: WebAuthn passwordless auth with role-based permissions
- **All config changes deploy instantly** - no code changes required

### CLI - Universal Agent Interface

The CLI executes any task, sends messages to agents, and invokes MCP tools in any environment. Enable local-to-remote and remote-to-remote agentic flows:

```bash
# Send a message to an agent
systemprompt admin agents message blog "Write a post about MCP security"

# List available MCP tools
systemprompt admin agents tools content-manager

# Execute from local to remote, or remote to remote
systemprompt cloud deploy --profile production
```

The same CLI runs locally during development and in production on your cloud instance - your AI can manage infrastructure from anywhere.

### Scheduling - Deterministic Tasks

Run scheduled jobs when you need predictable, time-based execution:

```yaml
# services/scheduler/daily-analytics.yaml
jobs:
  daily_report:
    cron: "0 9 * * *"
    task: "analytics:generate_daily_report"
    enabled: true
```

```bash
# List scheduled jobs
systemprompt infra jobs list

# Run a job manually
systemprompt infra jobs run daily_report

# View execution history
systemprompt infra jobs history
```

Scheduling complements agentic flows - use agents for dynamic reasoning and schedulers for deterministic tasks.

### Building Your Own Project

Use the [systemprompt-template](https://github.com/systempromptio/systemprompt-template) to create a new project with the recommended structure for agents, MCP servers, and content. The template includes installation instructions, example configurations, and a working development environment.

## Using as a Library

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://docs.rs/systemprompt/badge.svg)](https://docs.rs/systemprompt)

Build your own extensions by adding the facade to your `Cargo.toml`:

```toml
[dependencies]
systemprompt = { version = "0.0.1", features = ["full"] }
```

## Architecture

systemprompt.io uses a **layered crate architecture**:

```
┌─────────────────────────────────────────────────────────┐
│  ENTRY: api, cli                                        │
├─────────────────────────────────────────────────────────┤
│  APP: runtime, scheduler, generator, sync               │
├─────────────────────────────────────────────────────────┤
│  DOMAIN: users, oauth, ai, agent, mcp, files, content   │
├─────────────────────────────────────────────────────────┤
│  INFRA: database, events, security, config, logging     │
├─────────────────────────────────────────────────────────┤
│  SHARED: models, traits, identifiers, extension         │
└─────────────────────────────────────────────────────────┘
```

Dependencies flow downward only. Domain crates communicate via traits and events, not direct dependencies.

See [full architecture documentation](https://systemprompt.io/documentation/architecture) for details on all 25+ crates.

## Extension Framework

Extensions enable downstream projects to extend core functionality without modifying it.

```rust
use systemprompt_extension::*;

struct MyExtension;
impl Extension for MyExtension { ... }
impl ApiExtension for MyExtension { ... }

register_extension!(MyExtension);
register_api_extension!(MyExtension);
```

**Available extension traits:**

| Trait | Purpose |
|-------|---------|
| `Extension` | Base trait - ID, name, version, dependencies |
| `SchemaExtension` | Database table definitions |
| `ApiExtension` | HTTP route handlers |
| `ConfigExtensionTyped` | Config validation at startup |
| `JobExtension` | Background job definitions |
| `ProviderExtension` | Custom LLM/tool provider implementations |

Extensions are discovered at runtime via the `inventory` crate.

## License

FSL-1.1-ALv2 (Functional Source License) - see [LICENSE](LICENSE) for details.

## Links

- [Discord](https://discord.gg/wkAbSuPWpr) — Get help and connect with the community
- [Documentation](https://systemprompt.io/documentation) — Full guides and API reference
- [GitHub](https://github.com/systempromptio/systemprompt-core) — Source code and issues
- [Website](https://systemprompt.io) — Learn more about systemprompt.io
