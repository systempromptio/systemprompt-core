<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# Integration Tests

These suites prove the audited path holds end to end. Every crate here drives a production crate against a live PostgreSQL fixture and asserts the database state that results, not just the HTTP status that comes back.

## Layout

19 integration-test crates, each targeting one production crate, plus shared fixtures at [`crates/tests/common/`](../common/).

| Crate | Exercises |
|-------|-----------|
| `agent` | A2A protocol, tasks, messages |
| `analytics` | Session and request metrics |
| `api` | HTTP server surfaces |
| `cli` | CLI commands against a real binary |
| `cloud` | Cloud API and tenant flows |
| `content` | Content management |
| `database` | Pool, migrations, constraints, consistency |
| `events` | Event bus and SSE |
| `extension` | Extension registration and schemas |
| `files` | File storage |
| `gateway` | Provider routing and inference |
| `generator` | Static site generation |
| `mcp` | MCP server lifecycle and transports |
| `oauth` | OAuth2 / OIDC |
| `runtime` | AppContext and lifecycle |
| `scheduler` | Job scheduling |
| `security` | JWT and auth |
| `sync` | Cloud sync |
| `users` | User management |

Shared fixtures live in `crates/tests/common/`:

| Directory | Holds |
|-----------|-------|
| `fixtures/` | Bootstrap, DB pool, JWT minting, OAuth client seeding, PKCE pairs, system-admin fixture |
| `migrate/` | Fresh-database migration helpers |
| `mocks/` | Mock services and inference stubs |

The authoritative description of what each suite covers is the per-test docstring in `crates/tests/integration/<crate>/src/*.rs`.

## Running

Run from `crates/tests/`. The integration shard compiles and links a bounded slice of the workspace and spawns the real `systemprompt` binary.

```bash
just test-shard integration
```

Each run drops, recreates, and freshly migrates its target database. Override the target with `TEST_DATABASE_URL`; the default is a disposable `systemprompt_test`. Never point the suite at the dev `systemprompt-web` database. Its web-project triggers break core tests.

To run a single crate directly under nextest:

```bash
cargo nextest run -p systemprompt-database-integration-tests
```

---

Part of [systemprompt.io](https://systemprompt.io), self-hosted AI governance infrastructure.
</content>
</invoke>
