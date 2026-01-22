# systemprompt.io

**Production infrastructure for AI agents. Self-hosted or cloud.**

The missing layer between AI frameworks and production deployment. Not another SDK - complete infrastructure with authentication, permissions, and multi-agent orchestration built on open standards (MCP, A2A, OAuth2).

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg)](https://crates.io/crates/systemprompt)
[![Documentation](https://docs.rs/systemprompt/badge.svg)](https://docs.rs/systemprompt)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Table of Contents

- [Why systemprompt.io?](#why-systemprompt)
- [Quick Start](#quick-start)
- [Using as a Library](#using-as-a-library)
- [Architecture](#architecture)
- [Extension Framework](#extension-framework)
- [License](#license)

## Why systemprompt.io?

Frameworks give you building blocks. We give you the building.

| Problem | How others solve it | systemprompt.io |
|---------|---------------------|-----------------|
| Agent auth | Build it yourself | OAuth2/OIDC + WebAuthn built-in |
| User permissions | Build it yourself | Role-based, per-agent, per-tool scopes |
| MCP hosting | Run locally only | Production deployment with auth |
| Multi-agent | Orchestration libraries | A2A protocol with shared state |
| Deployment | Figure it out | One command to cloud or self-host |

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

### Prerequisites

- Rust 1.75+
- Docker (for local PostgreSQL) **OR** systemprompt.io Cloud account

### Install the CLI

**Option A: Install from crates.io**
```bash
cargo install systemprompt-cli
```

**Option B: Build from source**
```bash
git clone https://github.com/systempromptio/systemprompt-core
cd systemprompt-core
cargo build --release -p systemprompt-cli
```

### Setup

All setup is done through the CLI. Choose your database option:

#### Option 1: Local PostgreSQL (Free)

```bash
# Start PostgreSQL in Docker
docker run -d --name systemprompt-db \
  -e POSTGRES_DB=systemprompt \
  -e POSTGRES_USER=systemprompt \
  -e POSTGRES_PASSWORD=systemprompt \
  -p 5432:5432 \
  postgres:16

# Login to systemprompt.io Cloud (free account - enables CLI profile management)
systemprompt cloud auth login

# Create a local tenant with your Docker database
systemprompt cloud tenant create --type local

# Create and configure your profile
systemprompt cloud profile create local

# Run database migrations
systemprompt infra db migrate

# Start services
systemprompt infra services start --all
```

#### Option 2: systemprompt.io Cloud (Paid)

Production-ready agentic mesh served over the web. Cloud deployment includes your code and managed PostgreSQL running together as a complete platform. Point your DNS and deploy your web frontend chained to your agents.

```bash
# Login to systemprompt.io Cloud
systemprompt cloud auth login

# Create a cloud tenant (provisions your full platform instance)
systemprompt cloud tenant create --region iad

# Create and configure your profile
systemprompt cloud profile create production

# Deploy to cloud
systemprompt cloud deploy --profile production
```

Your agentic mesh will be deployed in the region of your choice and available at your tenant URL (e.g., `https://my-tenant.systemprompt.io`). This can be easily used (CNAME) to run your own web accessible agent mesh and domain.  

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

Use the [systemprompt-template](https://github.com/systempromptio/systemprompt-template) to create a new project with the recommended structure for agents, MCP servers, and content.

## Using as a Library

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

See [full architecture documentation](https://docs.systemprompt.io/architecture) for details on all 25+ crates.

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

## Versioning

Follows [Semantic Versioning](https://semver.org/):

- **Major**: Breaking API changes
- **Minor**: New features, backward compatible
- **Patch**: Bug fixes, backward compatible

Current version: **0.0.1**

## License

FSL-1.1-ALv2 (Functional Source License) - see [LICENSE](LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/systempromptio/systemprompt-core)
- [Issues](https://github.com/systempromptio/systemprompt-core/issues)
- [Documentation](https://docs.systemprompt.io)
- [Website](https://systemprompt.io)
