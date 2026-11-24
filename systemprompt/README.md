## What this crate is

`systemprompt` is the facade over the systemprompt-core workspace: one dependency and one feature matrix in place of thirty-odd `systemprompt-*` crates. Depend on it to build extensions, embed the governance pipeline, or drive the API server from your own binary.

For what the platform is and why it exists, see the [repository README](https://github.com/systempromptio/systemprompt-core#readme). To evaluate it end to end without writing code, start with the MIT-licensed [systemprompt-template](https://github.com/systempromptio/systemprompt-template), whose scripted demo suite runs every published claim on your laptop.

## Use as a library

```toml
[dependencies]
systemprompt = { version = "0.86.2", features = ["full"] }
```

A product binary is a composition of the core members and your own:

```rust
use std::process::ExitCode;

use systemprompt::{RunError, reference};

fn main() -> Result<ExitCode, RunError> {
    reference().with(MyExtension).run()
}
```

`MyExtension` implements `Extension` (`metadata` and `contribute`); `systemprompt/examples/extension.rs` is the smallest complete member, and [authoring extensions](https://github.com/systempromptio/systemprompt-core/blob/main/documentation/guides/authoring-extensions.md) is the guide. `Composition` and `reference()` need the `full` feature.

| Feature | Includes |
|---------|----------|
| `core` *(default)* | traits, models, wire, manifest, identifiers, the extension framework (`Extension`, `Contributions`) |
| `database` | PostgreSQL abstraction (`DbPool`) |
| `api` | HTTP server, `AppContext` and OAuth token issuance (requires `core` + `database`) |
| `cli` | CLI entry point and the command contract member commands implement |
| `full` | Everything: all domain modules, CLI, and `Composition` / `reference()` |
| `slack` | `systemprompt::slack` module (not in `full`; `api` compiles the crate regardless) |
| `teams` | `systemprompt::teams` module (not in `full`; `api` compiles the crate regardless) |

The full per-feature matrix, including the finer-grained flags (`config`, `mcp`, `events`, `security`, `runtime`, `content`, `analytics`, `notifications`, `brain`, and the rest), is on the [docs.rs crate root](https://docs.rs/systemprompt).

## Requirements

- Rust 1.94+ (the workspace is edition 2024).
- PostgreSQL 18+ for every feature above `core`.

Running the server rather than depending on it is covered in [getting-started.md](https://github.com/systempromptio/systemprompt-core/blob/main/documentation/getting-started.md).

## License

Business Source License 1.1 (BSL-1.1). Source-available for evaluation, testing, and non-production use; production use requires a commercial license. Each version converts to Apache-2.0 four years after its publication. You will always be able to read, audit, and eventually own this code. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE). Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

## Security

Report vulnerabilities to **ed@systemprompt.io**, not via public issues. See [SECURITY.md](https://github.com/systempromptio/systemprompt-core/blob/main/SECURITY.md).

---
