# systemprompt-brain-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-brain-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-brain-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-brain-entry?style=flat-square)](https://docs.rs/systemprompt-brain-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The brain routes of systemprompt.io: fact reads, search, person-entered facts, fact governance, dispute resolution and the proposal queue under `/api/v1/brain`, and the recall simulator under `/api/v1/admin/brain`.

**Layer**: Entry. The `http` feature carries the routes. `BrainHttpExtension` (`brain-http`) contributes them as two route sets (`user_routes()` behind the user gate, `admin_routes()` behind the admin gate, both under the `brain` rate limit), so the API server mounts them without naming this crate. Disabling `brain-http` in the profile unmounts both (404); disabling `brain` alone refuses boot. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
