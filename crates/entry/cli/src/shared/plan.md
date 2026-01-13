# Shared Module Compliance Plan

**Location:** `src/shared/`

## Current State

### Files

| File | Lines | Purpose |
|------|-------|---------|
| `mod.rs` | 31 | Module exports, `resolve_input` helper |
| `command_result.rs` | 217 | CommandResult types and render_result |
| `paths.rs` | 4 | Template constants |
| `process.rs` | 17 | Process execution utilities |
| `profile.rs` | 47 | Profile generation and saving |
| `project.rs` | 52 | ProjectRoot discovery |
| `docker.rs` | 50 | Docker build/push utilities |
| `web.rs` | 80 | Web asset building |

### Compliance Status: PASS

| Check | Status |
|-------|--------|
| mod.rs exists | PASS |
| plan.md exists | PASS |
| All files ≤300 lines | PASS |
| No inline comments | PASS |
| No doc comments | PASS |
| No unsafe | PASS |
| No println!/eprintln! | PASS |
| No unwrap()/expect() | PASS |
| No panic!/todo!/unimplemented! | PASS |
| No dead_code allows | PASS |

---

## Module Responsibilities

| Module | Responsibility |
|--------|----------------|
| `command_result` | CLI output types, artifact constructors, render_result |
| `paths` | Template file constants |
| `process` | Shell command execution |
| `profile` | Profile YAML generation and persistence |
| `project` | SystemPrompt project root discovery |
| `docker` | Docker image build, login, push |
| `web` | npm build, asset syncing |

---

## Key Types

### CommandResult<T>
Wraps command output with artifact metadata for MCP transformation.

### ArtifactType
Enum: Table, List, PresentationCard, Text, CopyPasteText, Chart, Form, Dashboard

### ProjectRoot
Discovers `.systemprompt` directory walking up from cwd.

---

## Dependencies

- `systemprompt_core_logging::CliService` - Output formatting
- `systemprompt_models::Profile` - Profile type
- `systemprompt_models::Config` - App configuration
