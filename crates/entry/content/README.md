# systemprompt-content-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-content-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-content-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-content-entry?style=flat-square)](https://docs.rs/systemprompt-content-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The content HTTP routes and CLI commands of systemprompt.io: published content, search, trackable links and the short-link redirect, the `core content` commands and sitemap generation.

**Layer**: Entry. The `http` feature carries the routes and the `cli` feature the commands; `ContentHttpExtension` contributes both to the composition, which the API server and the CLI mount. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `http` | `ContentHttpExtension` (id `content-http`, depends on `content`): the public `/api/v1/content` routes, link generation at `/api/v1/content/links/generate` for a signed-in user, and the `/r/{short_code}` redirect at the web root, all in the content-and-misc role group under the `content` rate limit. The routes read the content repositories the `content` extension provides. |
| `cli` | `core content` (contributed under `core`; a member command, so it needs a profile and is refused on a run given only `--database-url`) and `web sitemap` (contributed under `web`). |

Either feature links `systemprompt-generator`, whose `generator` extension owns the `content_prerender` and `page_prerender` jobs.

## Disabling

| Profile disables | Routes | Boot |
|------------------|--------|------|
| `content-http` | not mounted; the paths answer 404 | boots |
| `content` | — | refused: `content-http`, `generator` and `files` depend on it |
| `generator` | unchanged | boots; both prerender jobs are listed as inactive |
