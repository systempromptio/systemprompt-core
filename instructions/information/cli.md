# CLI Extension Guide

Extend systemprompt CLI from wrapper projects using manifest-based CLI plugins.

---

## Extension Points

| Method | Use Case | Command |
|--------|----------|---------|
| **CLI Plugin** | Custom CLI commands | `systemprompt ext run <binary> <args>` |
| **Jobs** | Scheduled/on-demand tasks | `systemprompt jobs run <job>` |
| **MCP Tools** | Agent-callable operations | `systemprompt mcp call <tool>` |
| **HTTP Routes** | Web endpoints | `curl POST /api/v1/ext/...` |
| **Database** | Data operations | `systemprompt db execute` |

---

## CLI Plugin Architecture

### manifest.yaml

```yaml
extension:
  type: cli
  name: My CLI Extension
  binary: my-cli
  description: Custom CLI commands
  enabled: true
  commands:
    - name: homepage
      description: Homepage management
    - name: content
      description: Content operations
```

### Usage

```bash
systemprompt ext list
systemprompt ext run my-cli homepage regenerate
systemprompt ext run my-cli content generate --type blog
systemprompt --json ext run my-cli homepage status
```

### Binary Implementation

```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use systemprompt_cli::shared::{render_result, CommandResult};
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};

#[derive(Parser)]
#[command(name = "my-cli")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(subcommand)]
    Homepage(HomepageCommands),
}

#[derive(Subcommand)]
enum HomepageCommands {
    Regenerate,
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    ProfileBootstrap::init()?;
    SecretsBootstrap::init()?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Homepage(cmd) => match cmd {
            HomepageCommands::Regenerate => {
                let result = CommandResult::text(Output { success: true });
                render_result(&result);
            }
            HomepageCommands::Status => {
                let result = CommandResult::card(Status { healthy: true });
                render_result(&result);
            }
        },
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct Output { success: bool }

#[derive(serde::Serialize)]
struct Status { healthy: bool }
```

---

## Environment Propagation

Core propagates to CLI extension binaries:

| Variable | Purpose |
|----------|---------|
| `SYSTEMPROMPT_PROFILE` | Profile path |
| `JWT_SECRET` | JWT signing secret |
| `DATABASE_URL` | Database connection |

---

## Product Binary Pattern

Wrapper projects link extensions via facade:

```rust
pub use systemprompt::*;
pub use my_extension as _;
```

```rust
use my_product as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
```

---

## Core Implementation

| File | Purpose |
|------|---------|
| `crates/shared/models/src/extension/mod.rs` | `Cli` type, `CliCommand`, `commands` field |
| `crates/infra/loader/src/extension_loader.rs` | `get_enabled_cli_extensions()`, `find_cli_extension()` |
| `crates/entry/cli/src/commands/ext/mod.rs` | `ExtCommands`: `List`, `Run` |

---

## Alternative Patterns

### Jobs

```rust
impl Extension for MyExtension {
    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![Arc::new(MyJob)]
    }
}
```

```bash
systemprompt jobs run my_job
```

### MCP Tools

```yaml
extension:
  type: mcp
  name: My MCP Server
  binary: my-mcp-server
```

```bash
systemprompt mcp call my-tool
```

### HTTP Routes

```rust
impl Extension for MyExtension {
    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        let router = Router::new().route("/action", post(handler));
        Some(ExtensionRouter::new("/api/v1/ext/my", router))
    }
}
```
