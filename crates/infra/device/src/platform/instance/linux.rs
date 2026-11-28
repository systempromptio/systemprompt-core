//! Linux: a Unix socket in the data root. A socket address holds 108 bytes
//! there. The abstract socket namespace is not used: it has no owner and no
//! mode, so any local user could reach it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(super) type Endpoint = super::unix::SocketEndpoint<108>;
