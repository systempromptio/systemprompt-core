# SystemPrompt CLI

**Layer:** Entry
**Binary:** `systemprompt`

Command-line interface for SystemPrompt OS. Every command supports both human-friendly interactive mode and agent-friendly non-interactive mode.

---

## Structure

```
src/
├── agents/             # systemprompt admin agents [...]
├── build/              # systemprompt build [...]
├── cloud/              # systemprompt cloud [...]
├── common/             # Common utilities
├── contexts/           # systemprompt core contexts [...]
├── db/                 # systemprompt infra db [...]
├── jobs/               # systemprompt infra jobs [...]
├── logs/               # systemprompt infra logs [...]
├── presentation/       # Output formatting
├── services/           # systemprompt infra services [...]
├── setup/              # systemprompt admin setup
├── web/                # systemprompt web [...]
├── shared/             # Cross-cutting infrastructure
│   ├── mod.rs
│   └── config.rs       # CliConfig, OutputFormat, VerbosityLevel
├── tui.rs              # TUI bootstrap (auth, profile selection, launch)
├── lib.rs              # CLI entrypoint and command routing
└── main.rs             # Binary entrypoint
```

Note: The actual TUI implementation lives in `crates/entry/tui/` (`systemprompt_core_tui`).
The `tui.rs` here is a bootstrap module that handles authentication and profile selection before launching the TUI.

---

# Part 1: Universal Rules

## 1.1 Function Signature (MANDATORY)

Every `execute` function MUST accept `config: &CliConfig`:

```rust
// ✅ CORRECT
pub async fn execute(cmd: Command, ctx: Arc<AppContext>, config: &CliConfig) -> Result<()>

// ❌ WRONG - Missing config
pub async fn execute(cmd: Command, ctx: Arc<AppContext>) -> Result<()>
```

**Why:** Without `config`, commands cannot:
- Check `config.is_interactive()` for mode-aware behavior
- Check `config.is_json_output()` for structured output
- Respect global `--non-interactive` flag

---

## 1.2 Dual-Mode Operation (MANDATORY)

Every command MUST support both modes:

| Mode | Audience | Requirements |
|------|----------|--------------|
| Interactive | Humans | Rich prompts, confirmations, colored output |
| Non-Interactive | Agents | All inputs via flags, JSON output, no prompts |

**Implementation pattern:**

```rust
pub async fn execute(args: Args, config: &CliConfig) -> Result<()> {
    let value = resolve_input(
        args.value,
        "value",
        config,
        || prompt_for_value(),
    )?;
    // ...
}
```

---

## 1.3 Forbidden Patterns

| Pattern | Why Forbidden | Resolution |
|---------|---------------|------------|
| `println!` | Bypasses output formatting | `CliService::info()` or `CliService::raw()` |
| `eprintln!` | Bypasses error handling | `CliService::error()` |
| `unwrap()` | Panics on error | `?` with `.context()` |
| `expect()` | Panics on error | `?` with `.context()` |
| `panic!()` | Crashes CLI | Return `Err(anyhow!(...))` |
| `env::set_var("SYSTEMPROMPT_NON_INTERACTIVE", ...)` | Overrides user's config | Let `CliConfig` control mode |
| Missing `config: &CliConfig` | Can't check interactive mode | Add to function signature |
| Prompt without flag | Blocks non-interactive mode | Add `--flag` equivalent |
| Destructive op without `--yes` | Forces interactive mode | Add `--yes` / `-y` flag |

**Validation command:**
```bash
# Run from any command folder
grep -r "println!" *.rs && echo "FAIL" || echo "PASS"
grep -r "\.unwrap()" *.rs | grep -v "unwrap_or" && echo "FAIL" || echo "PASS"
grep -r "env::set_var.*NON_INTERACTIVE" *.rs && echo "FAIL" || echo "PASS"
```

---

## 1.4 Required Patterns

### resolve_input Helper

```rust
fn resolve_input<T, F>(
    value: Option<T>,
    flag_name: &str,
    config: &CliConfig,
    prompt_fn: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    match value {
        Some(v) => Ok(v),
        None if config.is_interactive() => prompt_fn(),
        None => Err(anyhow!("--{} is required in non-interactive mode", flag_name)),
    }
}
```

### Confirmation Pattern

```rust
// For destructive operations
if !args.yes && config.is_interactive() {
    if !CliService::confirm("Delete all items?")? {
        CliService::info("Cancelled");
        return Ok(());
    }
}
```

### JSON Output Pattern

```rust
if config.is_json_output() {
    CliService::json(&data);
} else {
    CliService::key_value("Name", &data.name);
    CliService::key_value("Status", &data.status);
}
```

---

## 1.5 Standard Flags

| Flag | Short | Purpose | When Required |
|------|-------|---------|---------------|
| `--yes` | `-y` | Skip confirmation | Any destructive operation |
| `--dry-run` | | Preview without executing | Destructive or expensive operations |
| `--force` | | Override safety checks | Operations with safety guards |
| `--id` | | Resource identifier | When interactive selection exists |
| `--json` | | JSON output | Handled globally |
| `--non-interactive` | | Disable prompts | Handled globally |

---

## 1.6 Global Flags & Environment Variables

| Flag | Environment Variable | Purpose |
|------|---------------------|---------|
| `--non-interactive` | `SYSTEMPROMPT_NON_INTERACTIVE=1` | Disable prompts |
| `--json` | `SYSTEMPROMPT_OUTPUT_FORMAT=json` | JSON output |
| `--yaml` | `SYSTEMPROMPT_OUTPUT_FORMAT=yaml` | YAML output |
| `--quiet` | `SYSTEMPROMPT_LOG_LEVEL=quiet` | Minimal output |
| `--verbose` | `SYSTEMPROMPT_LOG_LEVEL=verbose` | Detailed output |
| `--no-color` | `NO_COLOR=1` | Disable colors |

---

## 1.7 Output Standards

```rust
use systemprompt_core_logging::CliService;

// Sections
CliService::section("Creating Tenant");
CliService::subsection("Database");

// Messages
CliService::info("Connecting...");
CliService::success("Created successfully");
CliService::warning("Port in use");
CliService::error("Connection failed");

// Data
CliService::key_value("ID", &id);
CliService::json(&data);
CliService::yaml(&data);
CliService::raw(&text);  // Unformatted output
```

---

# Part 2: Domain-Specific Rules

## 2.1 agents/

**Commands:** `systemprompt admin agents [agent|mcp] [...]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| `delete` command has `--yes` flag | Required |
| `status`/`list` commands support `--json` output | Required |
| No `env::set_var("SYSTEMPROMPT_NON_INTERACTIVE", ...)` | Required |

### Function Signatures

```rust
// mod.rs
pub async fn execute(cmd: AgentsCommands, ctx: Arc<AppContext>, config: &CliConfig) -> Result<()>

// agents.rs
pub async fn execute(cmd: AgentCommands, ctx: Arc<AppContext>, config: &CliConfig) -> Result<()>

// mcp.rs
pub async fn execute(cmd: McpCommands, ctx: Arc<AppContext>, config: &CliConfig) -> Result<()>
```

### Required Flags

| Command | Required Flags |
|---------|---------------|
| `agents agent delete` | `--yes` / `-y` |
| `agents agent delete --all` | `--yes` / `-y` (destructive) |

### JSON Output Required

| Command | JSON Structure |
|---------|---------------|
| `agents agent list` | `[{"id": "...", "status": "..."}]` |
| `agents agent status` | `[{"id": "...", "status": "...", "port": ...}]` |
| `agents mcp list` | `[{"name": "...", "status": "..."}]` |
| `agents mcp list-packages` | `["pkg1", "pkg2"]` |

---

## 2.2 build/

**Commands:** `systemprompt build [mcp]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| No interactive prompts | Compliant (no prompts needed) |

This domain is simple - no interactive prompts needed. Just ensure `config` is passed through.

---

## 2.3 cloud/

**Commands:** `systemprompt cloud [auth|profile|tenant|sync|secrets|deploy|status|restart|init|dockerfile]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| Interactive-only commands return clear error | Required |
| All prompts have flag equivalents | Required |
| All destructive operations have `--yes` | Required |

### Interactive-Only Commands

These commands CANNOT work non-interactively and must return clear errors:

```rust
// cloud/auth/login.rs
pub async fn execute(args: LoginArgs, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "OAuth login requires interactive mode.\n\n\
             Alternatives:\n\
             - systemprompt cloud auth set-token <TOKEN>\n\
             - Set SYSTEMPROMPT_CLOUD_TOKEN environment variable"
        ));
    }
    // ... OAuth flow
}
```

| Command | Reason | Alternative |
|---------|--------|-------------|
| `cloud auth login` | Browser OAuth | `set-token` or env var |
| `cloud checkout` | Payment flow | N/A |

### Required Flags by Command

| Command | Required Flags |
|---------|---------------|
| `cloud profile create` | `--tenant-id`, `--anthropic-key`, `--openai-key`, `--gemini-key` |
| `cloud profile edit` | `--host`, `--port`, `--environment`, `--log-level`, `--set key=value` |
| `cloud profile delete` | `--yes` |
| `cloud tenant create` | `--tenant-type`, `--db-host`, `--db-port`, `--db-user`, `--db-password`, `--db-name`, `--region` |
| `cloud tenant show` | `--id` |
| `cloud tenant delete` | `--id`, `--yes` |
| `cloud tenant edit` | `--id`, `--set key=value` |
| `cloud restart` | `--yes` |
| `cloud sync` (no subcommand) | Error in non-interactive - must specify subcommand |

### Environment Variable Support

| Flag | Environment Variable |
|------|---------------------|
| `--anthropic-key` | `ANTHROPIC_API_KEY` |
| `--openai-key` | `OPENAI_API_KEY` |
| `--gemini-key` | `GEMINI_API_KEY` |
| `--db-host` | `SYSTEMPROMPT_DB_HOST` |
| `--db-port` | `SYSTEMPROMPT_DB_PORT` |
| `--db-user` | `SYSTEMPROMPT_DB_USER` |
| `--db-password` | `SYSTEMPROMPT_DB_PASSWORD` |
| `--db-name` | `SYSTEMPROMPT_DB_NAME` |

---

## 2.4 logs/

**Commands:** `systemprompt infra logs [view|search|stream|export|cleanup|delete|trace|request]`

### Command Structure

```
logs
├── view [--tail N] [--level LEVEL] [--module MODULE] [--since DURATION]
├── search <PATTERN> [--level LEVEL] [--module MODULE] [--since DURATION] [-n LIMIT]
├── stream [--level LEVEL] [--module MODULE] [--interval MS] [--clear]
│   └── (alias: follow)
├── export [--format json|csv|jsonl] [--output FILE] [--since DURATION] [--limit N]
├── cleanup [--older-than DURATION | --keep-last-days N] [--dry-run] [-y]
├── delete [-y]
├── trace
│   ├── list [-n LIMIT] [--since DURATION] [--agent NAME] [--status STATUS]
│   └── show <ID> [--verbose] [--json] [--steps] [--ai] [--mcp] [--artifacts] [--all]
└── request
    ├── list [--limit N] [--since DURATION] [--model MODEL] [--provider PROVIDER]
    └── show <REQUEST_ID> [--messages] [--tools]
```

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| Destructive operations have `--yes` | Required |
| `--dry-run` for cleanup operations | Required |

### Required Flags

| Command | Required Flags |
|---------|---------------|
| `logs cleanup` | `--older-than` or `--keep-last-days`, `--yes` (or `--dry-run`) |
| `logs delete` | `--yes` |

---

## 2.5 services/

**Commands:** `systemprompt infra services [start|stop|restart|status|cleanup|serve]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| Port conflict handling has flag | Required |

### Required Flags

| Command | Required Flags |
|---------|---------------|
| `services serve` | `--kill-port-process` (for port conflicts) |

### Port Conflict Handling

```rust
let should_kill = args.kill_port_process ||
    (config.is_interactive() && CliService::confirm("Kill process using port?")?);

if should_kill {
    kill_process_on_port(port)?;
} else if !config.is_interactive() {
    return Err(anyhow!(
        "Port {} in use. Use --kill-port-process to terminate the process.",
        port
    ));
}
```

---

## 2.6 db/

**Commands:** `systemprompt infra db [query|execute|tables|describe|info|status|migrate|assign-admin|reset]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| `db reset` has `--yes` | Required |

### Required Flags

| Command | Required Flags |
|---------|---------------|
| `db reset` | `--yes` |

### JSON Output Required

| Command | JSON Structure |
|---------|---------------|
| `db tables` | `[{"name": "...", "row_count": ...}]` |
| `db info` | `{"version": "...", "tables": [...], "size": "..."}` |
| `db status` | `{"status": "connected", "version": "...", ...}` |

---

## 2.7 jobs/

**Commands:** `systemprompt infra jobs [list|run|cleanup-sessions|session-cleanup|log-cleanup]`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |

### JSON Output Required

| Command | JSON Structure |
|---------|---------------|
| `jobs list` | `[{"name": "...", "description": "...", "schedule": "...", "enabled": true}]` |

---

## 2.8 setup/

**Commands:** `systemprompt admin setup`

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| Full non-interactive support via flags | Required |
| Config file alternative | Required |

### Required Flags

```rust
#[derive(Args)]
pub struct SetupArgs {
    // Database
    #[arg(long, env = "SYSTEMPROMPT_DB_HOST")]
    pub db_host: Option<String>,
    #[arg(long, env = "SYSTEMPROMPT_DB_PORT", default_value = "5432")]
    pub db_port: u16,
    #[arg(long, env = "SYSTEMPROMPT_DB_USER")]
    pub db_user: Option<String>,
    #[arg(long, env = "SYSTEMPROMPT_DB_PASSWORD")]
    pub db_password: Option<String>,
    #[arg(long, env = "SYSTEMPROMPT_DB_NAME")]
    pub db_name: Option<String>,

    // Docker
    #[arg(long)]
    pub use_docker: bool,
    #[arg(long)]
    pub reuse_container: bool,

    // API Keys
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_key: Option<String>,
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_key: Option<String>,
    #[arg(long, env = "GEMINI_API_KEY")]
    pub gemini_key: Option<String>,

    // Options
    #[arg(long)]
    pub environment: Option<String>,
    #[arg(long)]
    pub skip_migrate: bool,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
}
```

### Non-Interactive Execution

```rust
pub async fn execute(args: SetupArgs, config: &CliConfig) -> Result<()> {
    if let Some(config_path) = &args.config {
        return execute_from_config(config_path).await;
    }

    if !config.is_interactive() {
        // Validate all required args
        let db_host = args.db_host.ok_or_else(|| anyhow!("--db-host required"))?;
        let db_user = args.db_user.ok_or_else(|| anyhow!("--db-user required"))?;
        // ... validate all required fields
        return execute_non_interactive(args).await;
    }

    execute_interactive(args).await
}
```

---

## 2.9 web/

**Commands:** `systemprompt web [content-types|templates|assets|sitemap|validate]`

### Command Structure

```
web
├── content-types           # Manage content types (from content config)
│   ├── list [--enabled|--disabled] [--category CATEGORY]
│   ├── show <NAME>
│   ├── create [--name NAME] [--path PATH] [--source-id ID] [--category-id ID]
│   ├── edit <NAME> [--set KEY=VALUE] [--enable|--disable] [--url-pattern PATTERN]
│   └── delete <NAME> [-y]
├── templates               # Manage HTML templates
│   ├── list [--missing]
│   ├── show <NAME> [--preview-lines N]
│   ├── create [--name NAME] [--content-types TYPES] [--content -|FILE]
│   ├── edit <NAME> [--add-content-type TYPE] [--remove-content-type TYPE] [--content -]
│   └── delete <NAME> [-y] [--delete-file]
├── assets                  # List and inspect static assets
│   ├── list [--type css|logo|favicon|font|image|all]
│   └── show <PATH>
├── sitemap                 # Sitemap operations
│   ├── show [--preview]
│   └── generate [--output PATH] [--base-url URL]
└── validate [--only config|templates|assets|sitemap]
```

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Required |
| All prompts have flag equivalents | Required |
| All destructive operations have `--yes` | Required |
| Template content supports stdin (`--content -`) | Required |

### Path Resolution

All paths are resolved from the profile:
- `profile.paths.content_config()` → Content sources configuration
- `profile.paths.web_config()` → Web service configuration
- `profile.paths.web_metadata()` → Web metadata
- `profile.paths.web_path_resolved()` → Web service root (templates, assets)

### Required Flags by Command

| Command | Required Flags (Non-Interactive) |
|---------|----------------------------------|
| `web content-types create` | `--name`, `--path`, `--source-id`, `--category-id` |
| `web content-types delete` | `--yes` |
| `web templates create` | `--name`, `--content-types` |
| `web templates delete` | `--yes` |
| `web sitemap generate` | `--base-url` (or from metadata) |

### Piping HTML Content

Templates support piping HTML via stdin:

```bash
# Create template with HTML from stdin
cat template.html | systemprompt web templates create --name blog-post --content-types blog --content -

# Update template HTML
echo "<html>...</html>" | systemprompt web templates edit blog-post --content -

# Copy from existing file
systemprompt web templates create --name new-template --content-types articles --content ./source.html
```

### JSON Output Required

| Command | JSON Structure |
|---------|---------------|
| `web content-types list` | `{"content_types": [{"name": "...", "enabled": true, ...}]}` |
| `web templates list` | `{"templates": [{"name": "...", "file_exists": true, ...}]}` |
| `web assets list` | `{"assets": [{"path": "...", "asset_type": "css", ...}]}` |
| `web sitemap show` | `{"routes": [...], "total_routes": N}` |
| `web validate` | `{"valid": true, "errors": [], "warnings": []}` |

### Validation Checks

The `web validate` command checks:
1. **Config**: Web and content configs exist and parse correctly
2. **Templates**: Each template entry has a corresponding HTML file
3. **Assets**: Referenced branding assets (logo, favicon) exist
4. **Sitemap**: URL patterns are valid, priorities in range 0.0-1.0

---

## 2.10 tui/

**Commands:** `systemprompt` (no subcommand)

### Requirements

| Requirement | Status |
|-------------|--------|
| Return clear error in non-interactive mode | Required |

```rust
pub async fn execute(config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "TUI requires interactive mode.\n\n\
             Use specific commands instead:\n\
             - systemprompt infra services status\n\
             - systemprompt cloud status\n\
             - systemprompt admin agents agent list"
        ));
    }
    // ... TUI code
}
```

---

## 2.11 contexts/

**Commands:** `systemprompt core contexts [list|show|create|edit|delete|use|new]`

### Command Structure

```
contexts
├── list                              # List all contexts with stats
├── show <ID|NAME>                    # Show context details
├── create [--name NAME]              # Create new context
├── edit <ID|NAME> --name NAME        # Rename a context
├── delete <ID|NAME> [-y]             # Delete a context
├── use <ID|NAME>                     # Set session's active context
└── new [--name NAME]                 # Create and switch (shortcut)
```

### Context Resolution

Contexts can be referenced by:
- Full UUID
- Partial UUID prefix (minimum 4 characters)
- Context name (exact or case-insensitive match)

### Requirements

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | Compliant |
| Destructive operations have `--yes` | Compliant |
| No interactive prompts without flag equivalents | Compliant |

### Required Flags

| Command | Required Flags |
|---------|---------------|
| `contexts delete` | `--yes` / `-y` |
| `contexts edit` | `--name` |

### JSON Output

| Command | JSON Structure |
|---------|---------------|
| `contexts list` | `{"contexts": [...], "total": N, "active_context_id": "..."}` |
| `contexts show` | `{"id": "...", "name": "...", "task_count": N, ...}` |
| `contexts create` | `{"id": "...", "name": "...", "message": "..."}` |
| `contexts use` | `{"id": "...", "name": "...", "message": "..."}` |
| `contexts new` | `{"id": "...", "name": "...", "message": "..."}` |

### Non-Interactive Examples

```bash
# List contexts as JSON
systemprompt --json contexts list

# Create context with name
systemprompt core contexts create --name "My Project"

# Switch context by partial ID
systemprompt core contexts use a1b2c3d4

# Switch context by name
systemprompt core contexts use "My Project"

# Delete context without confirmation
systemprompt core contexts delete a1b2c3d4 --yes

# Create and switch in one command
systemprompt core contexts new --name "New Session"
```

---

# Part 3: Validation & Architecture

## 3.1 Validation

See [validation.md](./validation.md) for:
- Automated validation script
- Manual review checklist
- CI integration

## 3.2 Rust Architecture

See [instructions/rust/rust.md](../../../instructions/rust/rust.md) for:
- Crate layer definitions (shared, infra, domain, app, entry)
- Dependency rules
- Testing policy

---

# Part 4: Artifact-Compatible Results (MANDATORY)

The CLI is wrapped by an MCP server. Every command MUST return artifact-compatible results that can be transformed into A2A artifacts with proper metadata.

---

## 4.1 Core Types

Every command returns `CommandResult<T>` instead of `Result<()>`:

```rust
use systemprompt_models::cli::{CommandResult, ResultMetadata};
use systemprompt_models::artifacts::types::ArtifactType;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ServiceStatusOutput {
    pub services: Vec<ServiceRow>,
    pub summary: Summary,
}

pub async fn execute() -> Result<CommandResult<ServiceStatusOutput>> {
    let output = get_service_status().await?;

    Ok(CommandResult::table(output)
        .with_title("Service Status")
        .with_hints(json!({
            "columns": ["service", "type", "status", "pid"],
            "sortable": true
        })))
}
```

---

## 4.2 CommandResult Structure

```rust
pub struct CommandResult<T: Serialize + JsonSchema> {
    pub data: T,
    pub metadata: ResultMetadata,
}

pub struct ResultMetadata {
    pub artifact_type: ArtifactType,
    pub rendering_hints: Option<serde_json::Value>,
    pub title: Option<String>,
    pub description: Option<String>,
}
```

---

## 4.3 Artifact Type Constructors

| Constructor | Artifact Type | Use Case |
|-------------|---------------|----------|
| `CommandResult::table(data)` | `Table` | Lists, status outputs, multi-row data |
| `CommandResult::list(data)` | `List` | Simple item lists |
| `CommandResult::card(data)` | `PresentationCard` | Single entity details, profiles |
| `CommandResult::text(data)` | `Text` | Plain text output |
| `CommandResult::copy_paste(data)` | `CopyPasteText` | Tokens, keys, content to copy |
| `CommandResult::chart(data, ChartType::Bar)` | `Chart` | Metrics, analytics |
| `CommandResult::form(data)` | `Form` | Configuration, settings |
| `CommandResult::dashboard(data)` | `Dashboard` | Multi-panel views |

---

## 4.4 Builder Methods

```rust
CommandResult::table(data)
    .with_title("Service Status")           // Human-readable title
    .with_description("Current service states")  // Optional description
    .with_hints(json!({                     // Type-specific rendering hints
        "columns": ["name", "status", "port"],
        "sortable": true,
        "filterable": true,
        "page_size": 25
    }))
```

---

## 4.5 Rendering Hints by Artifact Type

### Table Hints
```json
{
    "columns": ["name", "status", "port"],
    "sortable": true,
    "filterable": true,
    "page_size": 25,
    "column_types": {
        "port": "integer",
        "status": "string"
    }
}
```

### Chart Hints
```json
{
    "chart_type": "bar",
    "x_axis": { "field": "date", "type": "time" },
    "y_axis": { "field": "count", "type": "linear" }
}
```

### Presentation Card Hints
```json
{
    "theme": "gradient",
    "show_ctas": true
}
```

---

## 4.6 Output Type Requirements

All output types MUST:

1. **Derive required traits:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MyOutput { ... }
```

2. **Use typed identifiers:**
```rust
use systemprompt_identifiers::{TaskId, UserId};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TaskOutput {
    pub id: TaskId,        // NOT String
    pub user_id: UserId,   // NOT String
}
```

3. **Avoid Option for display fields** - use empty strings/vecs instead of None

---

## 4.7 CLI Entry Point Pattern

Each command has two functions:

```rust
// Core function - returns CommandResult (used by MCP wrapper)
pub async fn execute() -> Result<CommandResult<ServiceStatusOutput>> {
    let output = get_data().await?;
    Ok(CommandResult::table(output).with_title("Services"))
}

// CLI entry point - handles human/JSON rendering
pub async fn execute_cli(config: &CliConfig) -> Result<()> {
    let result = execute().await?;
    CliService::render_result(&result, config.output_format());
    Ok(())
}
```

---

## 4.8 Forbidden Patterns

| Pattern | Resolution |
|---------|------------|
| `execute() -> Result<()>` | Return `Result<CommandResult<T>>` |
| `CliService::table()` direct calls | Use `CommandResult::table()` + `render_result()` |
| Untyped JSON output | Define output struct with `JsonSchema` |
| Missing `artifact_type` | Use appropriate constructor |
| Hardcoded column names in renderer | Put in `rendering_hints` |

---

## 4.9 MCP Transformation Flow

```
CLI Command
    ↓
CommandResult<T>
    ↓
command_result_to_call_tool_result()
    ↓
CallToolResult { structured_content, content }
    ↓
McpToA2aTransformer::transform()
    ↓
Artifact { parts, metadata, extensions }
```

The `x-artifact-type` is embedded in `structured_content` and the output schema automatically.

---

# Part 5: Examples

## 5.1 Non-Interactive Agent Workflows

```bash
# List agents as JSON
systemprompt --json agents agent list

# Delete agent without confirmation
systemprompt --non-interactive agents agent delete myagent --yes

# Delete all agents
systemprompt --non-interactive agents agent delete --all --yes
```

## 5.2 Non-Interactive Cloud Workflows

```bash
# Create profile
systemprompt --non-interactive cloud profile create prod \
  --tenant-id abc123 \
  --anthropic-key sk-ant-xxx

# Create local tenant
systemprompt --non-interactive cloud tenant create \
  --tenant-type local \
  --db-host localhost \
  --db-user admin \
  --db-password secret \
  --db-name mydb

# Edit profile
systemprompt --non-interactive cloud profile edit prod \
  --host 0.0.0.0 \
  --port 8080
```

## 5.3 Non-Interactive Setup

```bash
# Full setup with flags
systemprompt admin setup --non-interactive \
  --db-host localhost \
  --db-user admin \
  --db-password secret \
  --db-name systemprompt \
  --anthropic-key sk-ant-xxx \
  --yes

# Setup with environment variables
export SYSTEMPROMPT_DB_HOST=localhost
export SYSTEMPROMPT_DB_USER=admin
export SYSTEMPROMPT_DB_PASSWORD=secret
export ANTHROPIC_API_KEY=sk-ant-xxx
systemprompt admin setup --non-interactive --yes

# Setup from config file
systemprompt admin setup --config setup.toml
```

## 5.4 JSON Output Parsing

```bash
# Get first tenant ID
systemprompt --json cloud tenant list | jq -r '.[0].id'

# Filter running services
systemprompt --json services status | jq '.[] | select(.status == "running")'

# Check agent health
systemprompt --json agents agent status | jq '.[] | select(.healthy == false)'
```
