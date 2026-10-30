# MCP toolsets: one server declared in YAML

A toolset is a named group of MCP tools compiled into a binary. A deployment in the services tree lists the toolsets it hosts and who may use each tool; core's generic host serves them. The server binary needs no `ServerHandler` of its own, no router code and no per-call authentication: it calls `serve`.

Implementation: the trait is `crates/domain/mcp/src/toolset/mod.rs`, the host is `crates/domain/mcp/src/host/`, the factory inventory and `serve` are `crates/app/runtime/src/mcp_host/`. YAML types are in `crates/shared/models/src/mcp/toolset.rs`.

## What the host does on every request

1. **Authenticates** through `enforce_rbac_from_registry`: the bearer JWT (or proxy-verified identity) is checked against the server's `oauth` block, and the governance authz hook runs for the server. Each outcome goes to the registered access recorders.
2. **`tools/list`** returns only the tools whose resolved access admits the caller.
3. **`tools/call`** refuses a tool outside the caller's access with an `invalid_request` error whose `data` is `{"reason": "forbidden", "tool": "<name>"}`, records a `forbidden` outcome, and never runs it. An allowed call records `used` and runs in the owning toolset through one `McpToolExecutor`. The executor records the execution and claims the intent.
4. **`get_info`** answers the deployment's `description` as the title, the default icons, the instructions of every hosted toolset joined in order, and the resources capability when any hosted toolset serves resources.

## Declaring a server

```yaml
mcp_servers:
  toolbox:
    server_type: internal
    binary: systemprompt-mcp-toolbox
    package: toolbox
    port: 5020
    enabled: true
    display_in_web: true
    removable: false
    description: Issue reports and platform administration
    tool_policy: allow
    oauth:
      required: true
      scopes: [user]
      audience: mcp
      client_id: null
    toolsets:
      - id: issues
      - id: admin
        access:
          scopes: [admin]
      - id: experimental
        enabled: false
    tools:
      grant_role:
        access:
          groups: [platform]
        tool_policy: prompt
```

- `toolsets[].id` names a registered factory. `enabled` defaults to `true`. `access` is optional.
- `tools.<name>.access` narrows one tool. `tools.<name>.tool_policy` overrides the server's `tool_policy` for that tool in the signed bridge manifest; the server's value is still published under `*`.
- `description` and `removable` (default `true`) are now part of the deployment rather than being dropped on load.

### Access resolution

The most specific declaration wins:

1. the tool's own `tools.<name>.access`,
2. else the `access` of the toolset that serves the tool,
3. else the server's `oauth.scopes`, which RBAC has already enforced, so the tool is visible to every authenticated caller.

An `access` block is satisfied by **any** match: a held scope that implies a listed scope (`admin` implies `user`), a listed role in the token, or a listed group. Groups are read per request from the subject-attribute provider registered for the `group` dimension. They are looked up only when a decision depends on one. With no such provider, no caller is in any group.

### Validation

| Check | Where | Result |
|---|---|---|
| `toolsets:` on an external server, an id listed twice, an empty id, an empty `access: {}` block | `Deployment::validate` (services load) | load error |
| an enabled toolset id no linked crate registers | startup validation | warning (the toolset may be linked only into its own MCP binary) |
| the same id when the host binary starts | `serve` / `build_server` | start fails: `McpHostError::UnknownToolset` |
| a `tools:` entry no hosted toolset serves; two toolsets serving one tool name | `ToolsetServer::new` | start fails: `ToolsetHostError::UnhostedTool` / `DuplicateTool` |

## Writing a toolset

```rust
use async_trait::async_trait;
use systemprompt::identifiers::McpServerId;
use systemprompt::mcp::host::{ToolsetBuildResult, submit_toolset};
use systemprompt::mcp::{CallToolResult, McpError, McpToolset, ToolsetCall};
use systemprompt::system::AppContext;

#[derive(Debug)]
struct AdminToolset { /* services the tools need */ }

#[async_trait]
impl McpToolset for AdminToolset {
    fn id(&self) -> &str { "admin" }

    fn instructions(&self) -> &str { "Platform administration. ..." }

    fn tools(&self) -> Vec<rmcp::model::Tool> { /* handler.tool_definition(..) per tool */ }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "users_list" => call.executor.execute(&UsersList, call.request, call.context, call.client).await,
            other => Err(McpError::invalid_params(format!("Unknown tool: '{other}'"), None)),
        }
    }
}

fn build(ctx: &AppContext, server: &McpServerId) -> ToolsetBuildResult {
    Ok(Some(Box::new(AdminToolset { /* from ctx */ })))
}

submit_toolset!("admin", build);
```

`ToolsetCall` carries the executor, the request, the authenticated `RequestContext` and the client profile. When `call` runs, the caller is already authenticated and admitted. A factory returns `Ok(None)` when the toolset is unavailable in this installation; it is skipped with a warning. An `Err` fails start.

A toolset that serves resources overrides `serves_resources`, `list_resources` and `read_resource`. `read_resource` answers `None` for a URI the toolset does not own, so the host asks the next toolset.

Core registers two toolsets: `issues` (wrapping `IssuesMcpTools`) and `documents` (wrapping `DocumentsMcpTools`). The libraries stay public for servers that compose them by hand.

## The server binary

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ctx = /* build Arc<AppContext> from the profile, as today */;
    let server_id = McpServerId::try_new(std::env::var("MCP_SERVICE_ID")?)?;
    systemprompt::mcp::host::serve(ctx, server_id).await?;
    Ok(())
}
```

`serve(ctx, server_id)` binds `0.0.0.0:<port:>` from the deployment. `serve_on(ctx, server_id, listener)` takes a listener the binary bound before a slow bring-up. `build_server(&ctx, &server_id)` returns the `ToolsetServer` for composing the router yourself.

The crates that register toolsets, recorders, the authz hook and subject providers must be linked into the binary. Inventory only discovers what is linked.

## Recording access

Core decides who may list and call; the installation chooses where outcomes are written. Implement `McpAccessRecorder` and register a factory:

```rust
use systemprompt::mcp::host::{McpAccessEvent, McpAccessOutcome, McpAccessRecorder, SharedMcpAccessRecorder};
use systemprompt::mcp::register_mcp_access_recorder;

#[derive(Debug)]
struct AuditRecorder { pool: DbPool }

#[async_trait]
impl McpAccessRecorder for AuditRecorder {
    async fn record(&self, event: McpAccessEvent<'_>) {
        // event.server, event.tool, event.user_id, event.outcome
        // (Authenticated | Used | Rejected { reason } | Forbidden { reason });
        // outcome.action() is "authenticated" | "used" | "rejected" | "forbidden".
    }
}

fn recorder(pool: &DbPool) -> SharedMcpAccessRecorder {
    std::sync::Arc::new(AuditRecorder { pool: pool.clone() })
}

register_mcp_access_recorder!(recorder);
```

Recording is best-effort: `record` returns nothing, so a failed audit write cannot fail the call. For `tools/list` and resource requests, `event.tool` is the method name (`tools/list`, `resources/list`, `resources/read`).
