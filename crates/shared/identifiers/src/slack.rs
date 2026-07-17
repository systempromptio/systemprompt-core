//! Typed identifiers for the Slack integration — the workspace (team),
//! channel, and end-user identifiers that Slack assigns. These are opaque
//! Slack-side strings (e.g. `T0123456789`, `C0ABCDEF`, `U0GHIJKL`); the
//! integration never mints them, only carries them through dispatch and the
//! federated-identity mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(SlackWorkspaceId);
crate::define_id!(SlackChannelId);
crate::define_id!(SlackUserId);
