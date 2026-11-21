# systemprompt-marketplace-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace-entry?style=flat-square)](https://docs.rs/systemprompt-marketplace-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The marketplace entry surface of systemprompt.io: the marketplace catalogue and plugin-file routes, and the `core marketplace`, `core plugins` and `core skills` commands.

**Layer**: Entry. The `http` feature carries the routes and the `cli` feature the commands; `MarketplaceHttpExtension` contributes both to the composition, which the API server mounts and the CLI kernel dispatches. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `http` | `MarketplaceHttpExtension` (id `marketplace-http`, requires the managed repository and catalogue cache `managed_resources` provides): the public `/api/v1/marketplace` routes (`/marketplace.json`, `/marketplaces`, `/marketplaces/{id}`, `/marketplaces/{id}/manifest.yaml`, `/plugins/{plugin_id}/{*path}`) in the content-and-misc role group under the `marketplace` rate-limit scope with the content budget. |
| `cli` | The `core marketplace` (`explain`, `import`), `core plugins` (`list`, `show`, `validate`, `generate`) and `core skills` (`list`, `show`) groups under the kernel's `core` group, each requesting `services-tree`, the one grant their reserved CLI rows list. They read the services tree only through that grant and the loaded services configuration only through the open `ServicesState` seed. `core plugins` and `core marketplace explain` run on the detached command kernel with no database; `core skills` reads managed skills through the owner-bound `Arc<OrganizationSkillResolver>` `managed_resources` provides, which the member requires, and lists disk skills only when it cannot be resolved. |

## Disabling

| Profile disables | Routes | Commands | Boot |
|------------------|--------|----------|------|
| `marketplace-http` | not mounted; the paths answer 404 | refused at dispatch | boots |
| `managed_resources` | — | — | refused: it is required, and `marketplace-http` requires the state it provides |
