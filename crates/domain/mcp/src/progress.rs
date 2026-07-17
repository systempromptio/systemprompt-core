//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::{Peer, RoleServer};
use std::future::Future;
use std::pin::Pin;

pub type ProgressCallback = Box<
    dyn Fn(f64, Option<f64>, Option<String>) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

#[must_use]
pub fn create_progress_callback(token: ProgressToken, peer: Peer<RoleServer>) -> ProgressCallback {
    Box::new(
        move |progress: f64, total: Option<f64>, message: Option<String>| {
            let token = token.clone();
            let peer = peer.clone();
            let fut: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(async move {
                let mut param = ProgressNotificationParam::new(token, progress);
                param.total = total;
                param.message = message;
                if let Err(e) = peer.notify_progress(param).await {
                    tracing::warn!(error = %e, "Failed to send progress notification");
                }
            });
            fut
        },
    )
}
