<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**CLI Reference**](https://github.com/systempromptio/systemprompt-core/tree/main/crates/entry/cli) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---


# Database CLI Commands

Direct, auditable access to the PostgreSQL your organization owns. Every query, migration, and schema diff runs through one command surface, scriptable and non-interactive, so automation and human operators share the same audited path.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `infra db query <sql>` | Execute SQL query (read-only) | `Table` | No (DB only) |
| `infra db execute <sql>` | Execute write operation (INSERT, UPDATE, DELETE) | `Table` | No (DB only) |
| `infra db tables` | List all tables with row counts and sizes | `Table` | No (DB only) |
| `infra db describe <table>` | Describe table schema with columns and indexes | `Table` | No (DB only) |
| `infra db info` | Show database information | `Card` | No (DB only) |
| `infra db migrate` | Run database migrations | `Text` | No (DB only) |
| `infra db migrate-down <extension> <count>` | Revert the most recently applied migrations for an extension | `Text` | No (DB only) |
| `infra db migrate-squash` | Squash an extension's migrations into a baseline at version 0 | `Text` | No (DB only) |
| `infra db migrations status` | Show migration status | `Table` | No (DB only) |
| `infra db migrations history <extension>` | Show migration history for an extension | `Table` | No (DB only) |
| `infra db migrate-plan [extension]` | Show pending migrations (dry-run / plan) | `Text` | No (DB only) |
| `infra db migrate-status [extension]` | Detailed introspectable migration status | `Table` | No (DB only) |
| `infra db migrate-repair [extension]` | Repair migration checksum drift | `Text` | No (DB only) |
| `infra db migrate-mark-applied` | Record a migration as already applied without running its SQL | `Text` | No (DB only) |
| `infra db assign-admin <user>` | Assign admin role to a user | `Text` | No (DB only) |
| `infra db status` | Show database connection status | `Card` | No (DB only) |
| `infra db validate` | Validate schema against expected tables | `Text` | No (DB only) |
| `infra db count <table>` | Get row count for a table | `Text` | No (DB only) |
| `infra db indexes` | List all indexes | `Table` | No (DB only) |
| `infra db size` | Show database and table sizes | `Table` | No (DB only) |
| `infra db doctor` | Diff live schema against extension declarations | `Table` | No (DB only) |

---

## Core Commands

### db query

Execute a read-only SQL query.

```bash
sp infra db query "SELECT * FROM users LIMIT 10"
sp --json infra db query "SELECT * FROM users LIMIT 10"
sp infra db query "SELECT COUNT(*) FROM user_sessions" --format json
sp infra db query "SELECT id, name FROM users WHERE status = 'active'"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<sql>` | Yes | SQL query to execute |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | | Maximum number of rows to return |
| `--offset` | | Number of rows to skip |
| `--format` | `table` | Output format: `table`, `json`, `yaml` |

**Output Structure:**
```json
{
  "columns": ["id", "name", "email", "created_at"],
  "rows": [
    {
      "id": "user_abc123",
      "name": "johndoe",
      "email": "john@example.com",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "row_count": 1,
  "execution_time_ms": 15
}
```

**Artifact Type:** `Table`

---

### db execute

Execute a write operation (INSERT, UPDATE, DELETE).

```bash
sp infra db execute "UPDATE users SET status = 'active' WHERE id = 'user_abc'"
sp infra db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '7 days'"
sp --json infra db execute "INSERT INTO settings (key, value) VALUES ('feature_x', 'enabled')"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<sql>` | Yes | SQL statement to execute |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--format` | `table` | Output format: `table`, `json`, `yaml` |

**Output Structure:**
```json
{
  "rows_affected": 5,
  "execution_time_ms": 25,
  "message": "Query executed successfully, 5 row(s) affected"
}
```

**Artifact Type:** `Table`

---

### db tables

List all tables in the database with row counts and sizes.

```bash
sp infra db tables
sp --json infra db tables
sp infra db tables --filter user
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--filter` | | Filter tables by name substring |

**Output Structure:**
```json
{
  "tables": [
    {
      "name": "users",
      "schema": "public",
      "row_count": 150,
      "size_bytes": 524288
    },
    {
      "name": "user_sessions",
      "schema": "public",
      "row_count": 1200,
      "size_bytes": 1048576
    }
  ],
  "total": 25
}
```

**Artifact Type:** `Table`

---

### db describe

Describe table schema with columns and indexes.

```bash
sp infra db describe <table-name>
sp --json infra db describe users
sp infra db describe user_sessions
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<table>` | Yes | Table name to describe |

**Output Structure:**
```json
{
  "table": "users",
  "row_count": 150,
  "columns": [
    {
      "name": "id",
      "type": "text",
      "nullable": false,
      "default": null,
      "primary_key": true
    },
    {
      "name": "name",
      "type": "character varying",
      "nullable": false,
      "default": null,
      "primary_key": false
    },
    {
      "name": "email",
      "type": "character varying",
      "nullable": false,
      "default": null,
      "primary_key": false
    }
  ],
  "indexes": [
    {"name": "users_pkey", "columns": ["id"], "unique": true},
    {"name": "users_email_key", "columns": ["email"], "unique": true}
  ]
}
```

**Artifact Type:** `Table`

---

### db info

Show database information.

```bash
sp infra db info
sp --json infra db info
```

**Output Structure:**
```json
{
  "version": "PostgreSQL 17.7 on x86_64-pc-linux-musl...",
  "database": "PostgreSQL",
  "size": "45.41 MB",
  "table_count": 85,
  "tables": ["users", "user_sessions", "..."]
}
```

**Artifact Type:** `Card`

---

### db migrate

Run database migrations.

```bash
sp infra db migrate
sp --json infra db migrate
sp infra db migrate --allow-checksum-drift
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--allow-checksum-drift` | `false` | Proceed even when applied migration checksums differ from disk |

**Migration Process:**
1. Loads all registered modules
2. Executes schema migrations in order
3. Creates/updates tables as needed
4. Reports results

**Output Structure:**
```json
{
  "modules_installed": ["database", "users", "mcp", "ai", "..."],
  "message": "Database migration completed successfully"
}
```

**Artifact Type:** `Text`

---

### db migrate-down

Revert the most recently applied migrations for an extension.

```bash
sp infra db migrate-down mcp 1
sp --json infra db migrate-down users 2
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<extension>` | Yes | Extension whose migrations to revert |
| `<count>` | Yes | Number of migrations to revert |

**Artifact Type:** `Text`

---

### db migrate-squash

Squash an extension's migrations `1..=N` into a single baseline at version 0. Dry-run by default; pass `--apply` to write.

```bash
sp infra db migrate-squash --extension mcp --through 12
sp infra db migrate-squash --extension mcp --through 12 --apply
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--extension` | | Extension to squash |
| `--through` | | Highest migration version to fold into the baseline |
| `--apply` | `false` | Write the squash (omit for a dry-run) |

**Artifact Type:** `Text`

---

### db migrations

Show migration status and history.

```bash
sp infra db migrations status
sp --json infra db migrations status
sp infra db migrations history mcp
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `status` (alias `list`) | Show migration status |
| `history <extension>` | Show migration history for an extension |

**Artifact Type:** `Table`

---

### db migrate-plan

Show pending migrations as a plan. Dry-run only, performs no database writes.

```bash
sp infra db migrate-plan
sp infra db migrate-plan mcp
sp --json infra db migrate-plan mcp
```

**Optional Arguments/Flags:**
| Argument/Flag | Description |
|---------------|-------------|
| `[extension]` | Restrict the plan to one extension |
| `--json` | Emit structured JSON |

**Artifact Type:** `Text`

---

### db migrate-status

Detailed, introspectable migration status: applied, pending, and drift.

```bash
sp infra db migrate-status
sp infra db migrate-status mcp
sp --json infra db migrate-status
```

**Optional Arguments/Flags:**
| Argument/Flag | Description |
|---------------|-------------|
| `[extension]` | Restrict the report to one extension |
| `--json` | Emit structured JSON |

**Artifact Type:** `Table`

---

### db migrate-repair

Repair migration checksum drift by re-applying edited migrations in place. Dry-run without `--apply`.

```bash
sp infra db migrate-repair
sp infra db migrate-repair mcp --apply
sp --json infra db migrate-repair mcp
```

**Optional Arguments/Flags:**
| Argument/Flag | Description |
|---------------|-------------|
| `[extension]` | Restrict the repair to one extension |
| `--apply` | Write the repair (omit for a dry-run) |
| `--json` | Emit structured JSON |

**Artifact Type:** `Text`

---

### db migrate-mark-applied

Record a migration as already applied without running its SQL.

```bash
sp infra db migrate-mark-applied --extension mcp --version 7
sp --json infra db migrate-mark-applied --extension mcp --version 7
```

**Required Flags:**
| Flag | Description |
|------|-------------|
| `--extension` | Extension owning the migration |
| `--version` | Migration version to mark applied |
| `--json` | Emit structured JSON |

**Artifact Type:** `Text`

---

### db assign-admin

Assign admin role to a user.

```bash
sp infra db assign-admin <user>
sp --json infra db assign-admin johndoe
sp --json infra db assign-admin john@example.com
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<user>` | Yes | Username or email |

**Output Structure:**
```json
{
  "user_id": "5ee65aa3-4f0a-47af-ab90-ac91d21fc227",
  "name": "johndoe",
  "email": "john@example.com",
  "roles": ["user", "admin"],
  "already_admin": false,
  "message": "Admin role assigned to user 'johndoe' (john@example.com)"
}
```

**Artifact Type:** `Text`

---

### db status

Show database connection status.

```bash
sp infra db status
sp --json infra db status
```

**Output Structure:**
```json
{
  "status": "connected",
  "version": "PostgreSQL 17.7 on x86_64-pc-linux-musl...",
  "tables": 85,
  "size": "45.41 MB"
}
```

**Artifact Type:** `Card`

---

### db validate

Validate database schema against expected tables.

```bash
sp infra db validate
sp --json infra db validate
```

**Output Structure:**
```json
{
  "valid": true,
  "expected_tables": 25,
  "actual_tables": 85,
  "missing_tables": [],
  "extra_tables": ["anomaly_thresholds", "banned_ips", "..."],
  "message": "Database schema is valid"
}
```

**Artifact Type:** `Text`

---

### db count

Get the row count for a single table.

```bash
sp infra db count users
sp --json infra db count user_sessions
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<table>` | Yes | Table name to count |

**Artifact Type:** `Text`

---

### db indexes

List all indexes, optionally scoped to one table.

```bash
sp infra db indexes
sp infra db indexes --table users
sp --json infra db indexes
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--table` | | Restrict output to one table's indexes |

**Artifact Type:** `Table`

---

### db size

Show database and per-table sizes.

```bash
sp infra db size
sp --json infra db size
```

**Artifact Type:** `Table`

---

### db doctor

Diff the live schema against extension declarations.

```bash
sp infra db doctor
sp --json infra db doctor
```

**Artifact Type:** `Table`

---

## Complete Database Management Flow Example

```bash
# Phase 1: Check connection status
sp --json infra db status

# Phase 2: View database info
sp --json infra db info

# Phase 3: List all tables
sp --json infra db tables

# Phase 4: Describe specific table
sp --json infra db describe users

# Phase 5: Run a query
sp --json infra db query "SELECT COUNT(*) as count FROM users"

# Phase 6: Review the migration plan, then run migrations
sp --json infra db migrate-plan
sp infra db migrate

# Phase 7: Validate schema and diff against declarations
sp --json infra db validate
sp --json infra db doctor

# Phase 8: Assign admin role
sp --json infra db assign-admin developer@example.com
```

---

## Query Examples

### Common Read Queries

```bash
# Count users by status
sp infra db query "SELECT status, COUNT(*) FROM users GROUP BY status"

# Get recent sessions
sp infra db query "SELECT * FROM user_sessions ORDER BY started_at DESC LIMIT 10"

# Find content by source
sp infra db query "SELECT id, slug, title FROM markdown_content WHERE source_id = 'blog'"

# Check AI request costs
sp infra db query "SELECT DATE(created_at), SUM(cost_microdollars) FROM ai_requests GROUP BY DATE(created_at) ORDER BY 1 DESC LIMIT 7"
```

### Common Write Operations

```bash
# Update user status
sp infra db execute "UPDATE users SET status = 'suspended' WHERE id = 'user_abc'"

# Clean old sessions
sp infra db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '30 days'"

# Update setting
sp infra db execute "UPDATE settings SET value = 'enabled' WHERE key = 'feature_x'"
```

---

## Error Handling

### Connection Errors

```bash
sp infra db status
# Error: Failed to connect to database. Check your profile configuration.
```

### Query Errors

```bash
sp infra db query "SELECT * FROM nonexistent_table"
# Error: Table or column not found: nonexistent_table

sp infra db query "INVALID SQL"
# Error: Query failed: Write query not allowed in read-only mode
```

### Table Not Found

```bash
sp infra db describe nonexistent
# Error: Table 'nonexistent' not found
```

---

## JSON Output

All commands support the `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json infra db tables | jq .

# Extract specific fields
sp --json infra db tables | jq '.tables[].name'
sp --json infra db describe users | jq '.columns[].name'
sp --json infra db info | jq '.table_count'

# Query and process results
sp --json infra db query "SELECT * FROM users LIMIT 5" | jq '.rows[].email'
```

---

## Compliance Checklist

- [x] All `execute` entry points accept `ctx: &CommandContext`
- [x] All output types derive `Serialize`, `Deserialize`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] No destructive operations (reset removed for safety)
- [x] User-friendly error messages
- [x] Schema validation via `infra db validate`
- [x] Table sizes and index information included


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
