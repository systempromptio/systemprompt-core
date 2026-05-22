# Getting started

Take a clean machine to a running, governed systemprompt-core server, and make your first request. This is one path; it works.

By the end you will have built the binary, generated a profile and database, started the API server, and confirmed it answers on its health and discovery endpoints.

## Prerequisites

Install these before you start.

| Requirement | Why | Notes |
|-------------|-----|-------|
| Rust toolchain | Builds the binary | The workspace is edition 2024, so it needs Rust 1.85 or newer. The repository pins a nightly toolchain in `rust-toolchain.toml`; `rustup` selects it automatically inside the checkout. |
| PostgreSQL 18+ | The only durable state | A local server, a container, or a managed instance you can reach. The setup step can provision a local database for you, or use one you already run. |
| `just` | Runs the build recipes | A command runner. Install from your package manager or from just.systems. |
| `git` | Clone the repository | — |

You also need a PostgreSQL superuser (or a role with `CREATEDB` and `CREATEROLE`) reachable on the host and port you will give to setup, so it can create the application role and database.

## 1. Get the source and build

Clone the repository and build the workspace. The first build downloads and compiles all dependencies and takes several minutes.

```bash
git clone https://github.com/systempromptio/systemprompt-core.git
cd systemprompt-core
just build
```

`just build` runs `cargo build --workspace`. The compiled binary is at `./target/debug/systemprompt`.

## 2. Generate a profile, database, and secrets

Run the setup command. It creates a `.systemprompt/` directory holding your profile and secrets, provisions a PostgreSQL role and database, writes the connection string into the secrets file, and runs the database migrations.

```bash
./target/debug/systemprompt admin setup \
  --environment local \
  --db-host localhost \
  --db-port 5432 \
  --migrate \
  --yes
```

What each flag does:

- `--environment local` names the profile `local`. Setup writes `.systemprompt/profiles/local/profile.yaml` and `.systemprompt/secrets/local.secrets.json`.
- `--db-host` / `--db-port` point at your PostgreSQL server. Both default to `localhost` and `5432` if omitted.
- `--migrate` applies the schema after the profile is written. Without it, run migrations separately (step 3).
- `--yes` skips the interactive confirmation, so the command runs unattended.

Setup derives the database role and database name from the environment (`systemprompt_local`) unless you override them with `--db-user` and `--db-name`, and generates a password if you do not pass `--db-password`. The generated connection string is written into the secrets file — the profile references the secrets file, it does not contain the credential.

If you do not want setup to provision a local database, supply an existing one with `--db-user`, `--db-password`, and `--db-name` pointing at a database you already created.

To preview without writing anything, add `--dry-run`.

## 3. Apply migrations (only if you skipped `--migrate`)

If you ran setup without `--migrate`, apply the schema now:

```bash
./target/debug/systemprompt infra db migrate
```

To preview the pending migrations without writing to the database:

```bash
./target/debug/systemprompt infra db migrate-plan
```

## 4. Start the server

Start the API server in the foreground. It binds to the host and port from your profile, which default to `127.0.0.1:8080`.

```bash
./target/debug/systemprompt infra services start --api
```

The process installs database schemas, mounts the HTTP routes, and then runs until you stop it with Ctrl-C. Leave it running and open a second terminal for the next step.

## 5. Make your first request

First, check liveness. `GET /health` runs a `SELECT 1` against the database and returns `200` when the process is up and the database is reachable:

```bash
curl -i http://127.0.0.1:8080/health
```

```
HTTP/1.1 200 OK
content-type: application/json
```

A `503` here means the server is up but cannot reach PostgreSQL — recheck the connection string in your secrets file and that the database is running.

Next, fetch the API root. `GET /api/v1` is the unauthenticated discovery document that lists the mounted surfaces:

```bash
curl -s http://127.0.0.1:8080/api/v1
```

The response is a JSON document describing the available endpoints (core, agents, MCP, OAuth, streaming). Returning it confirms the HTTP surface is mounted and serving.

You now have a running, migrated, governed server.

## Next steps

- [overview.md](overview.md) — what the platform is and how its pieces fit together.
- [guides/deploy-production.md](guides/deploy-production.md) — run it for availability, backup, and recovery.
- Explore the CLI: every domain is discoverable with `--help`, for example `./target/debug/systemprompt infra --help` or `./target/debug/systemprompt admin --help`.
