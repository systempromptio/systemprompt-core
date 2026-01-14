# DB Command Tech Debt & Enhancements

## Overview

This document outlines all fixes and enhancements needed for the `db` CLI command module to achieve full compliance with the CLI README standards.

---

## Part 1: Violations to Fix

### 1.1 CommandResult Pattern Not Implemented

**Current State:** All functions return `Result<()>`
**Required:** Functions should return `Result<CommandResult<T>>` per Part 4 of CLI README

**Files to modify:**
- `crates/entry/cli/src/commands/db/mod.rs`

**Changes:**
- Create typed output structs for each command
- Return `CommandResult<T>` with proper artifact types
- Use builder methods (`.with_title()`, `.with_hints()`)

### 1.2 JSON Output Schema Mismatches

| Command | Current Fields | Required Fields |
|---------|----------------|-----------------|
| `db info` | `path`, `size`, `version`, `tables` | Add `host`, `port`, `database`, `connections` |
| `db tables` | Plain array | Wrap in `{"tables": [...], "total": N}`, add `schema`, `size_bytes` |
| `db describe` | `data_type`, no indexes | Rename to `type`, add `indexes` array |
| `db execute` | Same as query | Add `rows_affected`, `operation`, `message` |
| `db assign-admin` | No JSON support | Add full JSON output |

### 1.3 assign-admin Missing JSON Output

**Location:** `db/mod.rs` - `execute_assign_admin` function
**Fix:** Check `config.is_json_output()` and return structured JSON

### 1.4 Raw Database Errors Exposed

**Current:** `Error: error returned from database: relation "x" does not exist`
**Required:** `Error: Table 'x' not found`

**Fix:** Wrap database errors with user-friendly messages using `.context()`

---

## Part 2: Tech Debt to Address

### 2.1 Unused Config Parameter

The main `execute` function takes `_config: &CliConfig` but individual functions call `get_global_config()` instead.

**Fix:** Pass config through to all functions consistently.

### 2.2 No Typed Output Structs

Output is built inline with `serde_json::json!()` macros.

**Fix:** Create proper structs with `Serialize`, `Deserialize`, `JsonSchema` derives.

### 2.3 db execute Output Misleading

Returns `"No data returned"` for INSERT/UPDATE/DELETE.

**Fix:** Return `rows_affected` count from database.

---

## Part 3: Enhancements to Add

### 3.1 Add `db validate` Command

Validate schema against expected migrations without modifying data.

```rust
#[command(about = "Validate database schema")]
Validate,
```

### 3.2 Add Index Information to `db describe`

Include indexes in the describe output as documented.

### 3.3 Add Connection Pool Stats to `db info`

Include active/idle/max connection information.

---

## Part 4: Implementation Plan

### Phase 1: Create Output Types Module

Create `crates/entry/cli/src/commands/db/types.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbStatusOutput {
    pub status: String,
    pub version: String,
    pub tables: usize,
    pub size: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbInfoOutput {
    pub version: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub size: String,
    pub tables: Vec<String>,
    pub connections: ConnectionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionStats {
    pub active: u32,
    pub idle: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbTablesOutput {
    pub tables: Vec<TableInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub row_count: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbDescribeOutput {
    pub table: String,
    pub row_count: i64,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbExecuteOutput {
    pub rows_affected: u64,
    pub execution_time_ms: u64,
    pub operation: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbMigrateOutput {
    pub migrations_run: usize,
    pub modules_installed: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbAssignAdminOutput {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DbValidateOutput {
    pub valid: bool,
    pub expected_tables: usize,
    pub actual_tables: usize,
    pub missing_tables: Vec<String>,
    pub extra_tables: Vec<String>,
    pub message: String,
}
```

### Phase 2: Update mod.rs

1. Add `mod types;` and `pub use types::*;`
2. Update each execute function to return proper types
3. Pass `config` through consistently
4. Wrap errors with user-friendly messages
5. Add `db validate` command

### Phase 3: Update Database Service

Modify `DatabaseAdminService` to return richer information:
- Add `get_table_sizes()` method
- Add `get_indexes()` method
- Add `get_connection_stats()` method
- Return `rows_affected` from execute queries

### Phase 4: Update README

Sync README documentation with actual implementation.

---

## Checklist

- [x] Create `types.rs` with all output structs
- [x] Update `mod.rs` to use typed outputs
- [x] Add `db validate` command
- [x] Fix JSON output for `assign-admin`
- [x] Add `rows_affected` for write operations
- [x] Wrap database errors with user-friendly messages
- [x] Pass config consistently (remove `get_global_config()` calls)
- [x] Add index information to `db describe`
- [x] Update README to match implementation
- [x] Test all commands with `--json` flag
- [x] Verify compliance with CLI README standards
