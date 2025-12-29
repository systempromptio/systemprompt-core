# Module Internal Structure Checklist

**Applies to:** All crates with internal layers (api, services, models, repository)

---

## Required Directories

- [ ] `api/routes/` for HTTP endpoints (not `api/rest/`)
- [ ] `services/` with hierarchical organization
- [ ] `models/` for domain types
- [ ] `repository/` for data operations

## File Placement

- [ ] Only `lib.rs`, `error.rs`, `mod.rs` at `src/` root
- [ ] No orphaned files at root
- [ ] No empty directories
- [ ] Every directory has `mod.rs`

## Naming

- [ ] Directories: `snake_case`
- [ ] No redundant suffixes (`auth/auth_service.rs` → `auth/validation.rs`)
- [ ] No `_service.rs`, `_repository.rs` suffixes
- [ ] Repository ops: `queries.rs`, `mutations.rs`

## Layering

- [ ] API → Services → Repository (never skip)
- [ ] No business logic in repository
- [ ] No SQL in services
- [ ] No direct repository calls from API

## Anti-Patterns

- [ ] No `api/rest/` (use `api/routes/`)
- [ ] No flat service sprawl (use subdirectories)
- [ ] No mixed patterns in same module
- [ ] No `helpers.rs` / `utils.rs` at root
