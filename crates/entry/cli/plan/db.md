# Database Domain Plan

## Purpose
The `db` domain provides database operations, statistics, table management, and administrative functions.

## CLI Structure
```
systemprompt db query <sql> [--format table|json|yaml]
systemprompt db execute <sql> [--format table|json|yaml]
systemprompt db tables
systemprompt db describe <table_name>
systemprompt db info
systemprompt db status
systemprompt db migrate
systemprompt db assign-admin <user>
systemprompt db reset --yes
```

## Files
```
commands/db/
└── mod.rs       # DbCommands enum and all database operations
```

## Commands

### query
Execute a read-only SQL query.

**Arguments:**
- `sql` - SQL query string

**Flags:**
- `--format` - Output format: table (default), json, yaml

**Example:**
```bash
systemprompt db query "SELECT * FROM users LIMIT 10"
systemprompt db query "SELECT COUNT(*) FROM sessions" --format json
```

### execute
Execute a write operation (INSERT, UPDATE, DELETE).

**Arguments:**
- `sql` - SQL statement

**Flags:**
- `--format` - Output format: table (default), json, yaml

**Example:**
```bash
systemprompt db execute "UPDATE users SET active = true WHERE id = 1"
```

### tables
List all database tables with row counts.

**Output:**
- Table name
- Row count
- Size (if available)

### describe
Show table schema and structure.

**Arguments:**
- `table_name` - Name of table to describe

**Output:**
- Column name
- Data type
- Nullable
- Default value
- Constraints

### info
Show comprehensive database information.

**Output:**
- Database version
- Total tables
- Database size
- Connection info

### status
Show database connection status.

**Output:**
- Connection status (OK/FAILED)
- Version
- Table count
- Size

### migrate
Run database migrations.

**Process:**
1. Load configuration
2. Connect to database
3. Load all modules via ModuleLoader
4. Install each module's migrations
5. Report success/failure

### assign-admin
Assign admin role to a user.

**Arguments:**
- `user` - User identifier (email or ID)

**Output:**
- Success: User promoted with new roles
- Already admin: Warning message
- Not found: Error message

### reset
Reset database (drop all tables and recreate).

**Flags:**
- `--yes` - Skip confirmation prompt (REQUIRED in non-interactive mode)

**Safety:**
- Requires explicit confirmation
- Warns about data loss

## Implementation Details

### DatabaseTool struct
```rust
struct DatabaseTool {
    ctx: AppContext,
    admin_service: DatabaseAdminService,
    query_executor: QueryExecutor,
}
```

### Dependencies
- `systemprompt_core_database::{DatabaseAdminService, QueryExecutor, QueryResult}`
- `systemprompt_core_users::{UserService, UserAdminService, PromoteResult}`
- `systemprompt_runtime::AppContext`
- `systemprompt_loader::ModuleLoader`

## JSON Output Support

All commands support `--json` global flag for structured output:

```bash
systemprompt --json db tables
systemprompt --json db info
systemprompt --json db status
```

## Error Handling

- Connection failures return clear error messages
- Query errors include SQL context
- Migration failures list which modules failed
