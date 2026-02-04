# CLI Command Results

All CLI commands must return `CommandResult<T>` for consistent MCP artifact conversion.

## CommandResult Structure

```rust
pub struct CommandResult<T> {
    pub data: T,
    pub artifact_type: ArtifactType,
    pub title: Option<String>,
    pub hints: Option<RenderingHints>,
}
```

## Artifact Types and Requirements

### Table

For tabular data with rows and columns.

```rust
CommandResult::table(output)
    .with_title("Users")
    .with_columns(vec!["id", "name", "email", "status"])
```

**Requirements:**
- `data` must contain a struct with an array field (e.g., `users: Vec<UserSummary>`)
- `hints.columns` must list field names that exist on array items
- Array items must be serializable structs with named fields

**Output structure:**
```json
{
  "data": { "users": [...], "total": 10 },
  "artifact_type": "table",
  "title": "Users",
  "hints": { "columns": ["id", "name", "email", "status"] }
}
```

### List

For navigable item lists with title, summary, and link.

```rust
CommandResult::list(output)
    .with_title("Playbooks")
```

**Requirements:**
- `data` must contain an array of items
- Each item should have: `title`, `summary`/`description`, and `link`/`url`/`id`

### Text

For plain text output.

```rust
CommandResult::text(TextOutput::new("Operation completed"))
    .with_title("Result")
```

### CopyPasteText

For code or content meant to be copied.

```rust
CommandResult::copy_paste(content)
    .with_title("Generated Code")
```

### Card (PresentationCard)

For detailed single-item display.

```rust
CommandResult::card(playbook_detail)
    .with_title("Playbook: guide_start")
```

### Chart

For data visualization.

```rust
CommandResult::chart(data, ChartType::Bar)
    .with_title("Usage Statistics")
```

### Dashboard

For multi-section displays.

```rust
CommandResult::dashboard(sections)
    .with_title("System Status")
```

## Migration from Legacy Pattern

Legacy commands output data directly:

```rust
// OLD - Do not use
if config.is_json_output() {
    CliService::json(&output);
}
```

Migrate to CommandResult:

```rust
// NEW - Required pattern
Ok(CommandResult::table(output)
    .with_title("Title")
    .with_columns(vec!["col1", "col2"]))
```

## Rendering Hints

```rust
pub struct RenderingHints {
    pub columns: Option<Vec<String>>,    // Table column names
    pub chart_type: Option<ChartType>,   // Bar, Line, Pie, Area
    pub theme: Option<String>,           // UI theme hint
    pub extra: HashMap<String, Value>,   // Additional hints
}
```

## Output Data Conventions

### For Tables

Data struct should have:
- A primary array field containing the rows
- Optional metadata fields (total, limit, offset)

```rust
pub struct UserListOutput {
    pub users: Vec<UserSummary>,  // Primary array - becomes table rows
    pub total: usize,
    pub limit: i64,
    pub offset: i64,
}
```

### For Lists

Array items should have fields mappable to ListItem:
- `title` or `name` → ListItem.title
- `description` or `summary` → ListItem.summary
- `url`, `link`, or `id` → ListItem.link

## MCP Conversion

CommandResult converts to MCP CliArtifact:

| CommandResult | CliArtifact |
|---------------|-------------|
| `::table()` | `CliArtifact::Table(TableArtifact)` |
| `::list()` | `CliArtifact::List(ListArtifact)` |
| `::text()` | `CliArtifact::Text(TextArtifact)` |
| `::copy_paste()` | `CliArtifact::CopyPasteText(CopyPasteTextArtifact)` |
| `::card()` | `CliArtifact::Text(TextArtifact)` |
| `::dashboard()` | `CliArtifact::Dashboard(DashboardArtifact)` |

The conversion extracts:
- Columns from `hints.columns`
- Rows from the first array field in `data`
- Title from `title`
