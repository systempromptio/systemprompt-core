# CLI to MCP Artifact Conversion

## Overview

CLI commands return `CommandResult<T>`. MCP servers need `CliArtifact`. This document defines the conversion architecture.

## Current State

```
CLI Command
    ↓
CommandResult<T> { data, artifact_type, title, hints }
    ↓
JSON serialization (stdout)
    ↓
MCP Server parses JSON
    ↓
??? conversion ???
    ↓
CliArtifact (Table | List | Text | ...)
    ↓
McpResponseBuilder
```

## Target Architecture

```
CLI Command
    ↓
CommandResult<T>
    ↓
JSON (stdout)
    ↓
MCP Server parses → CommandResultRaw { data: Value, artifact_type, title, hints }
    ↓
CommandResultRaw::to_cli_artifact() [CORE METHOD]
    ↓
CliArtifact
    ↓
McpResponseBuilder
```

## Implementation Plan

### Phase 1: Core Types (systemprompt-models)

Location: `/crates/shared/models/src/artifacts/cli.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResultRaw {
    pub data: serde_json::Value,
    pub artifact_type: ArtifactType,
    pub title: Option<String>,
    pub hints: Option<RenderingHints>,
}

impl CommandResultRaw {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error>;

    pub fn to_cli_artifact(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError>;
}
```

### Phase 2: Conversion Logic

#### Table Conversion

```rust
fn convert_table(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
    let columns = self.hints
        .as_ref()
        .and_then(|h| h.columns.as_ref())
        .ok_or(ConversionError::MissingColumns)?;

    let items = extract_array_from_value(&self.data)?;

    let table_columns: Vec<Column> = columns
        .iter()
        .map(|name| Column::new(name, ColumnType::String))
        .collect();

    let artifact = TableArtifact::new(table_columns, ctx)
        .with_rows(items);

    if let Some(title) = &self.title {
        artifact = artifact.with_title(title);
    }

    Ok(CliArtifact::table(artifact))
}

fn extract_array_from_value(value: &Value) -> Result<Vec<Value>, ConversionError> {
    // If value is array, return it
    if let Some(arr) = value.as_array() {
        return Ok(arr.clone());
    }

    // If value is object, find first array field
    if let Some(obj) = value.as_object() {
        for (_, v) in obj {
            if let Some(arr) = v.as_array() {
                return Ok(arr.clone());
            }
        }
    }

    Err(ConversionError::NoArrayFound)
}
```

#### List Conversion

```rust
fn convert_list(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
    let items = extract_array_from_value(&self.data)?;

    let list_items: Vec<ListItem> = items
        .iter()
        .filter_map(|item| {
            let title = item.get("title")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())?;

            let summary = item.get("summary")
                .or_else(|| item.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let link = item.get("link")
                .or_else(|| item.get("url"))
                .or_else(|| item.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            Some(ListItem::new(title, summary, link))
        })
        .collect();

    let artifact = ListArtifact::new(ctx).with_items(list_items);

    Ok(CliArtifact::list(artifact))
}
```

#### Text Conversion

```rust
fn convert_text(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
    let content = if let Some(msg) = self.data.get("message").and_then(|v| v.as_str()) {
        msg.to_string()
    } else {
        serde_json::to_string_pretty(&self.data)?
    };

    let mut artifact = TextArtifact::new(&content, ctx);

    if let Some(title) = &self.title {
        artifact = artifact.with_title(title);
    }

    Ok(CliArtifact::text(artifact))
}
```

### Phase 3: MCP Server Integration

Location: `/extensions/mcp/systemprompt/src/server.rs`

```rust
use systemprompt::models::artifacts::{CliArtifact, CommandResultRaw};

async fn handle_systemprompt_tool(...) -> Result<CallToolResult, McpError> {
    let output = cli::execute(&input.command, auth_token)?;

    if !output.success {
        return Ok(McpResponseBuilder::<()>::build_error(...));
    }

    let artifact_repo = McpArtifactRepository::new(&self.db_pool)?;

    // Parse and convert using CORE method
    let artifact = CommandResultRaw::from_json(&output.stdout)
        .and_then(|cmd| cmd.to_cli_artifact(ctx))
        .unwrap_or_else(|_| {
            // Fallback for legacy commands
            CliArtifact::text(TextArtifact::new(&output.stdout, ctx))
        });

    let artifact_type = artifact.artifact_type_str();

    McpResponseBuilder::new(artifact, SERVER_NAME, ctx, execution_id)
        .build_and_persist(output.stdout, &artifact_repo, artifact_type, None)
        .await
        .map_err(|e| McpError::internal_error(...))
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Missing columns hint for table artifact")]
    MissingColumns,

    #[error("No array found in data for table/list conversion")]
    NoArrayFound,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unsupported artifact type: {0}")]
    UnsupportedType(String),
}
```

## File Locations

| Component | Location |
|-----------|----------|
| CliArtifact enum | `/crates/shared/models/src/artifacts/cli.rs` |
| CommandResultRaw | `/crates/shared/models/src/artifacts/cli.rs` |
| Conversion logic | `/crates/shared/models/src/artifacts/cli.rs` |
| MCP server usage | `/extensions/mcp/systemprompt/src/server.rs` |

## Testing

```rust
#[test]
fn test_table_conversion() {
    let json = r#"{
        "data": { "users": [{"id": "1", "name": "Alice"}] },
        "artifact_type": "table",
        "title": "Users",
        "hints": { "columns": ["id", "name"] }
    }"#;

    let cmd = CommandResultRaw::from_json(json).unwrap();
    let artifact = cmd.to_cli_artifact(&test_ctx()).unwrap();

    assert!(matches!(artifact, CliArtifact::Table { .. }));
}
```

## Migration Path

1. Implement `CommandResultRaw` and conversion in core
2. Update MCP server to use core conversion
3. Migrate legacy CLI commands to use `CommandResult<T>`
4. Remove fallback once all commands migrated
