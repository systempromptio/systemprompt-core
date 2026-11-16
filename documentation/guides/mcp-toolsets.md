# MCP toolsets: one server declared in YAML

A toolset is a named group of MCP tools compiled into a binary. A deployment in the services tree lists the toolsets it hosts and who may use each tool; core's generic host serves them. The server binary needs no `ServerHandler` of its own, no router code and no per-call authentication: it calls `serve`.

Implementation: the trait is `crates/domain/mcp/src/toolset/mod.rs`, the host is `crates/domain/mcp/src/host/`, the `ToolsetSlot` contribution point is `crates/app/mcp-host/src/contract.rs` and `serve` is `crates/app/mcp-host/src/toolsets/serve.rs`. YAML types are in `crates/shared/models/src/mcp/toolset.rs`.

## What the host does on every request

1. **Authenticates** through `enforce_rbac_from_registry`: the bearer JWT (or proxy-verified identity) is checked against the server's `oauth` block, and the governance authz hook runs for the server. Each outcome goes to the contributed access recorders.
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

- `toolsets[].id` names a toolset an admitted member contributes. `enabled` defaults to `true`. `access` is optional.
- `tools.<name>.access` narrows one tool. `tools.<name>.tool_policy` overrides the server's `tool_policy` for that tool in the signed bridge manifest; the server's value is still published under `*`.
- `description` and `removable` (default `true`) are part of the deployment.

### Access resolution

The most specific declaration wins:

1. the tool's own `tools.<name>.access`,
2. else the `access` of the toolset that serves the tool,
3. else the server's `oauth.scopes`, which RBAC has already enforced, so the tool is visible to every authenticated caller.

An `access` block is satisfied by **any** match: a held scope that implies a listed scope (`admin` implies `user`), a listed role in the token, or a listed group. Groups are read per request from the subject-attribute provider contributed for the `group` dimension. They are looked up only when a decision depends on one. With no such provider, no caller is in any group.

### Validation

| Check | Where | Result |
|---|---|---|
| `toolsets:` on an external server, an id listed twice, an empty id, an empty `access: {}` block | `Deployment::validate` (services load) | load error |
| an enabled internal server with neither `binary:` nor toolsets | startup validation | validation error |
| an enabled toolset id no admitted member contributes, on an enabled internal server with no `binary:` | building `ToolsetSlot` | composition refused: `UnregisteredToolsets` |
| a listed toolset id no admitted member contributes when the host starts | `serve` / `build_server` | start fails: `McpHostError::UnknownToolset` |
| a `tools:` entry no hosted toolset serves; two toolsets serving one tool name | `ToolsetServer::new` | start fails: `ToolsetHostError::UnhostedTool` / `DuplicateTool` |

## Writing a toolset

```rust
use async_trait::async_trait;
use systemprompt::identifiers::McpServerId;
use systemprompt::extension::prelude::*;
use systemprompt::mcp::host::{ToolsetBuildResult, ToolsetSlot};
use systemprompt::mcp::{CallToolResult, McpError, McpToolset, ToolsetCall};
use systemprompt::system::HostContext;

#[derive(Debug)]
struct AdminToolset { /* services the tools need */ }

#[async_trait]
impl McpToolset for AdminToolset {
    fn id(&self) -> &str { "admin" }

    fn instructions(&self) -> &'static str { "Platform administration. ..." }

    fn tools(&self) -> Vec<rmcp::model::Tool> { /* handler.tool_definition(..) per tool */ }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "users_list" => call.executor.execute(&UsersList, call.request, call.context, call.client).await,
            other => Err(McpError::invalid_params(format!("Unknown tool: '{other}'"), None)),
        }
    }
}

fn build(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    Ok(Some(Box::new(AdminToolset { /* from host */ })))
}

// in the member's `Extension::contribute`:
// c.slot::<ToolsetSlot>().toolset("admin", build);
```

`ToolsetCall` carries the executor, the request, the authenticated `RequestContext`, the client profile and the verified hop token of the call. When `call` runs, the caller is already authenticated and admitted. `toolset(id, build)` takes a `fn(&HostContext, &McpServerId) -> ToolsetBuildResult`; two toolsets with one id refuse the slot build. A build function returns `Ok(None)` when the toolset is unavailable in this installation; it is skipped with a warning. An `Err` fails start.

A toolset that serves resources overrides `serves_resources`, `list_resources` and `read_resource`. `read_resource` answers `None` for a URI the toolset does not own, so the host asks the next toolset.

Core contributes two toolsets: `issues` (wrapping `IssuesMcpTools`) and `documents` (wrapping `DocumentsMcpTools`). The libraries stay public for servers that compose them by hand.

## The server binary

An enabled internal server with `toolsets:` and no `binary:` is hosted by the composed binary itself: the API spawns `admin mcp serve --server <id>`, which admits the composition against the profile the API granted it and serves the server's toolsets.

```rust
pub async fn serve(ctx: Arc<HostContext>, server_id: McpServerId) -> Result<(), McpHostError>
```

`HostContext::from_process_env(admit)` boots from the environment the API spawned the server with: the profile path, the database URLs, the public verification key set and the secrets the deployment grants under `secrets:`; `admit` is the composition's admission (`AdmitFn`), run once against that profile. It holds no signing key, so a toolset that needs a signed document link asks the API for it through `host.link_issuer(&server_id)?`, passing the call's hop token; the API signs only for a server that declares the `documents` toolset (an upload link also admits `issues`). Rebuild every MCP binary against the core the API runs.

`serve(host, server_id)` binds `127.0.0.1:<port:>` from the deployment. `serve_on(host, server_id, listener)` takes a listener the binary bound before a slow bring-up. `build_server(&host, &server_id)` returns the `ToolsetServer` for composing the router yourself.

The toolsets, access recorders and UI renderers a host serves are the built `ToolsetSlot`, `AccessRecorderSlot` and `UiSlot` of its admitted members: a member that is not composed, or is disabled in the profile, contributes nothing.

## Recording access

Core decides who may list and call; the installation chooses where outcomes are written. Implement `McpAccessRecorder` and contribute a factory to `systemprompt::mcp::contract::AccessRecorderSlot`:

```rust
use systemprompt::extension::prelude::*;
use systemprompt::mcp::contract::AccessRecorderSlot;
use systemprompt::mcp::host::{McpAccessEvent, McpAccessOutcome, McpAccessRecorder, SharedMcpAccessRecorder};

#[derive(Debug)]
struct AuditRecorder { db: std::sync::Arc<dyn systemprompt::traits::DatabaseHandle> }

#[async_trait]
impl McpAccessRecorder for AuditRecorder {
    async fn record(&self, event: McpAccessEvent<'_>) {
        // event.server, event.tool, event.user_id, event.outcome
        // (Authenticated | Used | Rejected { reason } | Forbidden { reason });
        // outcome.action() is "authenticated" | "used" | "rejected" | "forbidden".
    }
}

// in the member's `Extension::contribute`; the factory runs once, against the resolved state:
// c.slot::<AccessRecorderSlot>().recorder(|r: &Resolved<'_>| -> Result<SharedMcpAccessRecorder, FactoryError> {
//     Ok(std::sync::Arc::new(AuditRecorder { db: r.db() }))
// });
```

Recording is best-effort: `record` returns nothing, so a failed audit write cannot fail the call. For `tools/list` and resource requests, `event.tool` is the method name (`tools/list`, `resources/list`, `resources/read`).
