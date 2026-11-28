//! macOS: a Unix socket in the data root. A socket address holds 104 bytes
//! there, four fewer than on Linux, so a data root that fits on one may not
//! fit on the other.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(super) type Endpoint = super::unix::SocketEndpoint<104>;
