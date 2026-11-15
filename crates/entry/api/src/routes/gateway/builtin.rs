//! The path prefixes the gateway's built-in surface serves, relative to the
//! gateway base.
//!
//! [`builtin_gateway_prefixes`] is the one list both the surface and the
//! registration check read: `gateway_mount_parts`
//! confines the built-in inference and bridge routers to it (with the mount's
//! `confined` layer) (a request path
//! outside every prefix answers 404, so a built-in route added without its
//! prefix fails its own route tests rather than escaping the check), and
//! `admit_registrations` refuses a registered route group whose path overlaps
//! one of these prefixes with `MountError::GatewayRouteShadowsBuiltin`. `/` is
//! the exact gateway root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::modules::ApiPaths;

#[must_use]
pub fn builtin_gateway_prefixes() -> Vec<&'static str> {
    let mut prefixes = vec![
        "/",
        "/models",
        "/messages",
        "/responses",
        "/chat/completions",
        "/me/provider-credentials",
        ApiPaths::OTEL,
    ];
    prefixes.extend(
        [
            ApiPaths::BRIDGE_AUTH_PAT,
            ApiPaths::BRIDGE_AUTH_SESSION,
            ApiPaths::BRIDGE_AUTH_SESSION_PAT,
            ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT,
            ApiPaths::BRIDGE_AUTH_CAPABILITIES,
            ApiPaths::BRIDGE_PUBKEY,
            ApiPaths::BRIDGE_PROFILE,
            ApiPaths::BRIDGE_WHOAMI,
            ApiPaths::BRIDGE_MANIFEST,
            ApiPaths::BRIDGE_PLUGINS,
            ApiPaths::BRIDGE_DEVICE,
            ApiPaths::BRIDGE_HEARTBEAT,
            ApiPaths::BRIDGE_STREAM,
            ApiPaths::BRIDGE_LATEST,
            ApiPaths::BRIDGE_DOWNLOAD,
            ApiPaths::BRIDGE_DRIVE_MANIFEST,
            ApiPaths::BRIDGE_DRIVE_CHANGES,
            ApiPaths::BRIDGE_DRIVE_BLOB,
            ApiPaths::BRIDGE_DRIVE_COMMIT,
            ApiPaths::BRIDGE_DRIVE_PRESENCE,
            ApiPaths::BRIDGE_DRIVE_PROPOSALS,
            ApiPaths::BRIDGE_DRIVE_TRASH,
            ApiPaths::BRIDGE_DRIVE_MOVE,
            ApiPaths::BRIDGE_DRIVE_TOKEN,
        ]
        .map(ApiPaths::gateway_relative),
    );
    prefixes
}
