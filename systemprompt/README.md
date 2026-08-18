<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# systemprompt

The governance engine behind AI infrastructure you actually own. One Rust binary, one PostgreSQL, every agent and tool call through one audited path. This is the facade crate: it re-exports the systemprompt-core workspace behind feature flags.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18+](https://img.shields.io/badge/postgres-18+-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

[**Website**](https://systemprompt.io) · [**Documentation**](https://github.com/systempromptio/systemprompt-core/blob/main/documentation/overview.md) · [**Evaluation template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

## What this crate is

`systemprompt` is the facade over the systemprompt-core workspace: one dependency and one feature matrix in place of thirty-odd `systemprompt-*` crates. Depend on it to build extensions, embed the governance pipeline, or drive the API server from your own binary.

For what the platform is and why it exists, see the [repository README](https://github.com/systempromptio/systemprompt-core#readme). To evaluate it end to end without writing code, start with the MIT-licensed [systemprompt-template](https://github.com/systempromptio/systemprompt-template), whose scripted demo suite runs every published claim on your laptop.

## Use as a library

```toml
[dependencies]
systemprompt = { version = "0.31.0", features = ["full"] }
```

```rust
use systemprompt::prelude::*;
```

| Feature | Includes |
|---------|----------|
| `core` *(default)* | traits, models, identifiers, extension |
| `database` | PostgreSQL abstraction (`DbPool`) |
| `api` | HTTP server and `AppContext` (requires `core` + `database`) |
| `cli` | CLI entry point |
| `full` | Everything: all domain modules + CLI |
| `slack` | Slack integration (opt-in; not included in `full`) |
| `teams` | Microsoft Teams integration (opt-in; not included in `full`) |

The full per-feature matrix, including the finer-grained flags (`config`, `mcp`, `events`, `security`, `runtime`, and the rest), is on the [docs.rs crate root](https://docs.rs/systemprompt).

## Requirements

- Rust 1.94+ (the workspace is edition 2024).
- PostgreSQL 18+ for every feature above `core`.

Running the server rather than depending on it is covered in [getting-started.md](https://github.com/systempromptio/systemprompt-core/blob/main/documentation/getting-started.md).

## License

Business Source License 1.1 (BSL-1.1). Source-available for evaluation, testing, and non-production use; production use requires a commercial license. Each version converts to Apache-2.0 four years after its publication. You will always be able to read, audit, and eventually own this code. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE). Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

## Security

Report vulnerabilities to **ed@systemprompt.io**, not via public issues. See [SECURITY.md](https://github.com/systempromptio/systemprompt-core/blob/main/SECURITY.md).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt)** · **[docs.rs](https://docs.rs/systemprompt)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Rent your control plane and you rent your audit trail. This one compiles.</sub>
</div>
