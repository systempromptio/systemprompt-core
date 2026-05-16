# Audit: systemprompt-cli — Area 6 (commands/web/ + commands/build/)

Scope: `crates/entry/cli/src/commands/web/**` and `crates/entry/cli/src/commands/build/**`.
Entry binary crate — `anyhow` permitted; per-item `///` banned.

## Checklist

1. **Layering** — clean. Only depends downward (config, models, logging, loader, generator); no sideways/circular deps.
2. **Error model** — clean. `anyhow::Result` throughout with `.context()`; appropriate for entry crate.
3. **No panics** — remediated. Removed `unreachable!()` in `content_types/edit.rs` by extracting a `sitemap_mut` helper and flattening the `apply_sitemap_set` re-match into `apply_set_key`. No `unwrap`/`expect`/`panic!`/`dbg!` elsewhere.
4. **Raw SQL** — clean. No SQL in scope; sitemap/content data sourced from YAML config files.
5. **File size** — clean. Largest file 261 lines (`content_types/edit.rs`), under the 300-line limit.
6. **Function size** — clean. All functions within ~75-line guidance.
7. **Async traits** — clean. No traits defined; commands are sync `fn`.
8. **Typed identifiers** — clean. `SourceId`/`CategoryId` constructed via `Id::new`; no `.into()`/`::from()` at call sites. Raw `String` names are config-map keys, not entity IDs.
9. **Comment standard** — clean. No `///` per-item rustdoc; no narrative `//` comments.
10. **No legacy** — remediated. Removed unused `WebBuildOutput` struct from `build/types.rs` (dead code, no constructor or reference).
11. **Naming** — clean. No `*Manager`; uses plain free functions and `CliService`.
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — remediated. Extracted three duplicated helpers into cohesive modules: `content_types/selection.rs` (`prompt_content_type_selection`, was 3x), `templates/selection.rs` (`prompt_template_selection`, was 3x), `assets/asset_type.rs` (`determine_asset_type`, was 2x).
14. **CHANGELOG** — observation only; not edited.

## Verification

- `SQLX_OFFLINE=true cargo clippy -p systemprompt-cli --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-cli --no-deps` — clean.
