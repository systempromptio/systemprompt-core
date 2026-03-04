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
                let _ = peer
                    .notify_progress(ProgressNotificationParam {
                        progress_token: token,
                        progress,
                        total,
                        message,
                    })
                    .await;
            });
            fut
        },
    )
}
