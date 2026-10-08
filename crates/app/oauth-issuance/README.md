# systemprompt-oauth-issuance

[![Crates.io](https://img.shields.io/crates/v/systemprompt-oauth-issuance.svg?style=flat-square)](https://crates.io/crates/systemprompt-oauth-issuance)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-oauth-issuance?style=flat-square)](https://docs.rs/systemprompt-oauth-issuance)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Issues OAuth 2.0 tokens for the `/oauth/token` endpoint: grant-type dispatch, authorization-code redemption, refresh-token rotation, client-credentials, RFC 8693 token exchange with delegated `act` chains, and the RFC 7523 jwt-bearer redemption of an ID-JAG.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

A token grant crosses domains: clients, codes and refresh tokens live in `systemprompt-oauth`, owners and delegates come from the user provider, every minted token is bound to an analytics session, and signing uses the `systemprompt-security` key authority under the profile's JWT settings. This crate composes them into one workflow. The HTTP surface (form extraction, HTTP Basic client authentication, the RFC 6749 error body) stays in `systemprompt-api`, which calls `TokenIssuanceOrchestrator` and maps `IssuanceError` onto the wire.

## Modules

| Module | Purpose |
|--------|---------|
| `orchestrator` | `TokenIssuanceOrchestrator`: parses `grant_type` and runs the matching grant |
| `request` | `TokenRequest` (grant input), `TokenResponse` (issued token), `RequestOrigin` (headers and caller IP for session analytics) |
| `user_tokens` | Access/refresh-token minting for user-bound grants, carrying the refresh-token family forward |
| `client_credentials` | Client acting as itself; scope and audience authorization against the client grant and its owner |
| `token_exchange` | Subject-token and ID-JAG validation, delegate resolution, `act`-chain assembly, ID-JAG issuance from upstream OIDC tokens |
| `validation` | Required-field extraction and authorization-code redemption |
| `error` | `IssuanceError`, `IssuanceResult` |

## Usage

```toml
[dependencies]
systemprompt-oauth-issuance = "0.64"
```

```rust
use systemprompt_oauth_issuance::{RequestOrigin, TokenIssuanceOrchestrator};

let origin = RequestOrigin { headers: &headers, caller_ip };
let response = TokenIssuanceOrchestrator::new(&oauth_state)
    .issue(request, origin)
    .await?;
```

## License

Business Source License 1.1. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).
