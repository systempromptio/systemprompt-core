# systemprompt-marketplace-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace-entry?style=flat-square)](https://docs.rs/systemprompt-marketplace-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The marketplace HTTP routes of systemprompt.io: the marketplace catalogue and plugin files.

**Layer**: Entry. The `http` feature carries the routes; `MarketplaceHttpExtension` contributes them to the composition, which the API server mounts. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `http` | `MarketplaceHttpExtension` (id `marketplace-http`, requires the managed repository and catalogue cache `managed_resources` provides): the public `/api/v1/marketplace` routes (`/marketplace.json`, `/marketplaces`, `/marketplaces/{id}`, `/marketplaces/{id}/manifest.yaml`, `/plugins/{plugin_id}/{*path}`) in the content-and-misc role group under the `marketplace` rate-limit scope with the content budget. |
| `cli` | No commands; the `core marketplace`, `core plugins` and `core skills` commands are served by the CLI kernel. |

## Disabling

| Profile disables | Routes | Boot |
|------------------|--------|------|
| `marketplace-http` | not mounted; the paths answer 404 | boots |
| `managed_resources` | — | refused: it is required, and `marketplace-http` requires the state it provides |
