# systemprompt-files-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-files-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-files-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-files-entry?style=flat-square)](https://docs.rs/systemprompt-files-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The file CLI commands of systemprompt.io: listing, inspecting, searching, uploading, validating and deleting stored files, the upload configuration and storage statistics, and the AI-generated image views.

**Layer**: Entry. The `cli` feature carries the commands; `FilesHttpExtension` (id `files-http`) contributes them to the composition, which the CLI mounts. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `cli` | `core files` (contributed under `core`), with the `core files ai` sub-group. `list`, `show`, `search` and `stats` also run on an invocation given only `--database-url`; `upload`, `delete`, `validate`, `config` and `ai` need a profile. The commands resolve the `Arc<FileRepository>` the `files` member provides and the kernel's `Arc<FilesConfig>` and `Arc<dyn FileStorage>`. |
| `http` | No routes. `/api/v1/files` stays reserved to the kernel, and stored files are served by the kernel's `/files/*` static fallback. |

## Disabling

| Profile disables | Commands | Boot |
|------------------|----------|------|
| `files-http` | `core files` refused at dispatch | boots |
| `files` | — | refused: `files` is a required member |
