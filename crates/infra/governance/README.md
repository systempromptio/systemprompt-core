# systemprompt-governance

[![Crates.io](https://img.shields.io/crates/v/systemprompt-governance.svg?style=flat-square)](https://crates.io/crates/systemprompt-governance)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-governance?style=flat-square)](https://docs.rs/systemprompt-governance)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Authorization rules, governance policies, secret scanning and decision auditing.

**Layer**: Infra. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Modules

| Module | Purpose |
|--------|---------|
| `authz` | Unified authorization decision plane: the deny-overrides `resolve`, the `access_control_rules` repository, the `AuthzDecisionHook` surface with built-in impls, ingestion, and Postgres audit sinks. |
| `policy` | Tool-use governance (`GovernancePolicy`, `GovernanceEngine`, `PolicyContext`) producing the same `Decision` shape the authz resolver returns. |

The `authz` extension (schema + migrations for `access_control_*`, `governance_*` and `approval_requests`) is registered from this crate under the id `authz`.

### `authz`

Unified authorization decision plane shared by the gateway `/v1/messages` proxy and the MCP RBAC middleware.

| Export | Type | Purpose |
|--------|------|---------|
| `resolve` | Fn | Deny-overrides resolver against `access_control_rules` |
| `AuthzDecisionHook` | Trait | Pluggable decision surface; concrete impl is carried on `AppContext` as `SharedAuthzHook` |
| `AllowAllHook` / `DenyAllHook` / `WebhookHook` | Struct | Built-in `AuthzDecisionHook` implementations |
| `AccessControlRepository` | Struct | CRUD over `access_control_rules` |
| `AccessControlIngestionService` | Struct | Loads rule sets from configuration |
| `AuthzExtension` | Struct | Registers schemas and migrations via the extension framework |
| `AuthzAuditSink` / `DbAuditSink` / `NullAuditSink` | Trait + impls | Sinks for governance decision audit records |
| `GovernanceDecisionRepository` | Struct | Reads `governance_decisions` audit rows |
| `Access` / `AccessRule` / `AuthzDecision` / `AuthzRequest` / `Decision` / `EntityKind` / `RuleType` | Types | Authz request and decision data model |
| `build_authz_hook` | Fn | Constructs the active `SharedAuthzHook` (Deny/Allow/Webhook) from `governance.authz` for the caller to store on its context |
| `SharedAuthzHook` | Type alias | `Arc<dyn AuthzDecisionHook>` — the value threaded through DI |

### `policy`

Shared tool-use governance types that produce the same `Decision` shape as the authz resolver.

| Export | Type | Purpose |
|--------|------|---------|
| `GovernancePolicy` | Trait | Contract every tool-call policy in the chain implements |
| `GovernanceEngine` | Struct | Ordered set of policies (scope check, secret scan, blocklist, rate limit) evaluated to one `Decision` |
| `PolicyContext` / `McpToolInput` / `AgentScope` | Types | Inputs a policy evaluates against |

## Usage

```toml
[dependencies]
systemprompt-governance = "0.74"
```

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
