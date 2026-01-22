# systemprompt.io

**Extensible AI agent orchestration framework**

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg)](https://crates.io/crates/systemprompt)
[![Documentation](https://docs.rs/systemprompt/badge.svg)](https://docs.rs/systemprompt)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Table of Contents

- [Why systemprompt.io?](#why-systemprompt)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Architecture](#architecture)
- [Available Crates](#available-crates)
- [Extension Framework](#extension-framework)
- [License](#license)

## Why systemprompt.io?

The new stack for the agentic age - infrastructure and application as one.

- **Infrastructure + Application**: Not just a framework - a complete runtime combining Web API, agent processes, and MCP servers with shared authentication and database
- **Open Standards**: Built on A2A framework, AGUI, and MCP protocols
- **Secure by Design**: OAuth2/OIDC, user management, and process isolation for agents and tools
- **Native Rust Performance**: Zero-cost abstractions and async-first design built on Tokio
- **Self-Hosted or Cloud**: Run locally on Docker, or deploy to your own isolated VM with included multi-tenant database
- **Open Source Foundation**: Extensible architecture with compile-time safe extensions
- **100% Extensible**: Build proprietary Rust wrappers on top of the core platform

## Quick Start

### Prerequisites

- Rust 1.75+
- Docker (for local PostgreSQL) **OR** systemprompt.io Cloud account

### Installation

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

# Login to systemprompt.io Cloud (required for CLI functionality)
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

Your agentic mesh will be deployed in the region of your choice and available at your tenant URL (e.g., `https://my-tenant.systemprompt.io`). This can we easily used (CNAME) to run your own web accessible agent mesh and domain.  

### Native MCP Client Support

Works out of the box with any MCP client - Claude Code, Claude Desktop, ChatGPT, and more. Use agent-native interfaces to directly manage your mesh in production with no extra infrastructure required.

```bash
# Your deployed agents are accessible via MCP
# Connect from Claude Code, Claude Desktop, or any MCP-compatible client
```

### Building Your Own Project

Use the [systemprompt-template](https://github.com/systempromptio/systemprompt-template) to create a new project with the recommended structure for agents, MCP servers, and content.

## Installation

Add the facade to your `Cargo.toml`:

```toml
[dependencies]
systemprompt = { version = "0.0.1", features = ["full"] }
```

## Architecture

systemprompt.io uses a **layered crate architecture** with strict dependency rules:

```
crates/
├── shared/           # Pure types, zero internal dependencies
│   ├── traits/       # Core trait definitions (LlmProvider, ToolProvider, Job)
│   ├── models/       # Data models, API types, config structs
│   ├── identifiers/  # Typed IDs (UserId, TaskId, etc.)
│   ├── client/       # HTTP client for external APIs
│   └── extension/    # Extension framework for customization
│
├── infra/            # Stateless infrastructure utilities
│   ├── database/     # SQLx abstraction, connection pooling
│   ├── events/       # Event bus, broadcasters, SSE
│   ├── security/     # JWT validation, token handling
│   ├── config/       # Configuration loading
│   ├── logging/      # Tracing setup, log sinks
│   ├── cloud/        # Cloud API, tenant management
│   └── loader/       # Module loader
│
├── domain/           # Bounded contexts with SQL + repos + services
│   ├── users/        # User identity (User, Role)
│   ├── oauth/        # Authentication (Token, Client, Grant, Session)
│   ├── files/        # File storage (File, FileMetadata)
│   ├── analytics/    # Metrics & tracking (Session, Event, Metric)
│   ├── content/      # Content management (Content, Category, Tag)
│   ├── ai/           # LLM integration (Request, Response, Provider)
│   ├── mcp/          # MCP protocol (Server, Tool, Deployment)
│   └── agent/        # A2A protocol (Agent, Task, Context, Skill)
│
├── app/              # Orchestration, no business logic
│   ├── runtime/      # AppContext, StartupValidator, lifecycle
│   ├── scheduler/    # Job scheduling, cron execution
│   ├── generator/    # Static site generation
│   └── sync/         # Sync services
│
└── entry/            # Entry points (binaries, public APIs)
    ├── api/          # HTTP gateway, route handlers, middleware
    └── cli/          # Command-line interface

systemprompt/         # Umbrella crate: Public API for external consumers
```

### Dependency Flow

```
UMBRELLA (systemprompt) ← External consumers
        │
    ENTRY (api, cli)
        │
    APP (runtime, scheduler, generator, sync)
        │
    DOMAIN (users, oauth, ai, agent, mcp, ...)
        │
    INFRA (database, events, security, config, logging, cloud)
        │
    SHARED (models, traits, identifiers, client, extension)
```

Domain crates cannot depend on each other - use traits or events for cross-domain communication.

## Available Crates

### Shared Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-traits` | Core trait definitions |
| `systemprompt-models` | Data models and config types |
| `systemprompt-identifiers` | Typed ID generators |
| `systemprompt-client` | HTTP client |
| `systemprompt-extension` | Extension framework |
| `systemprompt-provider-contracts` | Provider trait contracts |
| `systemprompt-template-provider` | Template provider traits |

### Infrastructure Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-database` | SQLx abstraction (PostgreSQL) |
| `systemprompt-events` | Event bus, SSE infrastructure |
| `systemprompt-security` | JWT, auth utilities |
| `systemprompt-config` | Configuration loading |
| `systemprompt-logging` | Tracing setup |
| `systemprompt-cloud` | Cloud API, tenant management |
| `systemprompt-loader` | Module loader |

### Domain Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-users` | User management |
| `systemprompt-oauth` | OAuth2/OIDC authentication |
| `systemprompt-files` | File storage |
| `systemprompt-analytics` | Metrics and tracking |
| `systemprompt-content` | Content management |
| `systemprompt-ai` | LLM integration |
| `systemprompt-mcp` | MCP protocol implementation |
| `systemprompt-agent` | Agent orchestration (A2A) |
| `systemprompt-templates` | Template registry |

### Application Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-runtime` | AppContext, lifecycle management |
| `systemprompt-scheduler` | Job scheduling |
| `systemprompt-generator` | Static site generation |
| `systemprompt-sync` | Sync services |

### Entry Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-api` | HTTP server framework |
| `systemprompt-cli` | Command-line interface |

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
