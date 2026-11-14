# systemprompt-teams-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-teams-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-teams-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-teams-entry?style=flat-square)](https://docs.rs/systemprompt-teams-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The Microsoft Teams inbound HTTP routes of systemprompt.io: Bot Framework activities, verified in the handler and dispatched to governed A2A agents.

**Layer**: Entry. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

The crate registers the `teams` extension: its config prefix and schema, and, behind the `http` feature, the route declaration for `/api/v1/teams/messages`. The routes mount in the `Messaging` route group with a handler-verified policy, so the handler checks the Bot Connector bearer token (RS256, bound to the activity `serviceUrl`) before it acts; there is no session context or caller-type gate in front of them. A verified message is dispatched through the `MessagingDispatcher` the agent capability binds into the extension state. A profile that disables the `teams` extension leaves the routes unmounted. The `cli` feature carries no commands.

The protocol logic (payload types, verification primitives, the outbound client and reply rendering) lives in `systemprompt-teams`.
