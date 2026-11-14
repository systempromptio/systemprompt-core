# systemprompt-slack-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-slack-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-slack-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-slack-entry?style=flat-square)](https://docs.rs/systemprompt-slack-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The Slack inbound HTTP routes of systemprompt.io: Events API, slash commands and Block Kit interactions, verified in the handler and dispatched to governed A2A agents.

**Layer**: Entry. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

The crate registers the `slack` extension: its config prefix and schema, and, behind the `http` feature, the route declaration for `/api/v1/slack/events`, `/api/v1/slack/commands` and `/api/v1/slack/interactivity`. The routes mount in the `Messaging` route group with a handler-verified policy, so the handler checks the Slack request signature (HMAC over the raw body) before it acts; there is no session context or caller-type gate in front of them. A verified message is dispatched through the `MessagingDispatcher` the agent capability binds into the extension state. A profile that disables the `slack` extension leaves the routes unmounted. The `cli` feature carries no commands.

The protocol logic (payload types, verification primitives, the outbound client and reply rendering) lives in `systemprompt-slack`.
