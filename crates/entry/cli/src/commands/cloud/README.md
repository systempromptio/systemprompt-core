# Cloud CLI Commands

Command reference for cloud. Use the installed command’s `--help` output for its complete arguments and defaults.

1. **Local project management** — initializing the `.systemprompt/` project
   structure, creating tenants backed by a local Docker Postgres or an
   external database, authoring profiles, and generating a Dockerfile. None
   of this requires a systemprompt.io Cloud account.
2. **Managed-cloud deployment** — authenticating against the systemprompt.io
   control plane and deploying a built image to a managed tenant. Only
   `auth`, `deploy`, `backup`, `status`, and cloud-tenant operations need a
   login.

## Command Reference

| Command | Description | Requires cloud login |
|---------|-------------|----------------------|
| `cloud auth login` | Authenticate via browser OAuth (GitHub or Google) | — |
| `cloud auth logout` | Clear credentials, tenant index, and tenant sessions | No |
| `cloud auth whoami` (alias `status`) | Show current authentication and token status | No |
| `cloud init` | Initialize project structure | No |
| `cloud tenant` | Interactive tenant menu | No |
| `cloud tenant create` | Create a tenant (local Docker or external Postgres) | No |
| `cloud tenant list / show / edit / delete` | Manage stored tenants | No |
| `cloud tenant rotate-credentials` | Rotate a managed tenant's database credentials | Yes |
| `cloud profile` | Interactive profile menu | No |
| `cloud profile create / list / show / edit / delete` | Manage profiles | No |
| `cloud deploy` | Build and deploy the image to a managed tenant | Yes |
| `cloud deploy --check` / `cloud doctor` | Run the pre-deploy preflight only | No |
| `cloud backup` | Download the tenant's runtime `services/` tree | Yes |
| `cloud status` | Check managed-tenant deployment status | Yes |
| `cloud dockerfile` | Generate a Dockerfile from discovered extensions | No |

## Authentication

`cloud auth login` is interactive-only: it opens a browser for OAuth (GitHub
or Google), receives the token on a loopback callback, verifies it against
the control plane, and persists:

- `.systemprompt/credentials.json` — the operator token (`0600`).
- `.systemprompt/tenants.json` — the tenant index synced from the account.

`cloud auth logout` removes both files and every tenant-scoped CLI session;
local sessions are untouched.

After creating a local profile, run `systemprompt admin bootstrap` to ensure
the profile's configured `system_admin.username` exists in the database with
the admin role. Service startup fails if that user is missing.

## Deployment

`cloud deploy` runs the doctor preflight, builds the Docker image locally,
fetches registry credentials from the control plane, pushes the image,
provisions the profile's secrets to the tenant, and triggers the deploy.
`--check` stops after the preflight and needs no login. `--skip-push` reuses
the last pushed image.

## JSON Output

All commands support the global `--json` flag for structured output:

```bash
systemprompt --json cloud auth whoami | jq .
systemprompt --json cloud tenant list | jq '.tenants[].id'
```

---
