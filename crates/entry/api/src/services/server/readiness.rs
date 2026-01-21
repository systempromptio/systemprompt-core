use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tokio::sync::broadcast;

static API_READY: AtomicBool = AtomicBool::new(false);
static READINESS_SENDER: OnceLock<broadcast::Sender<ReadinessEvent>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub enum ReadinessEvent {
    ApiReady,
    ApiShuttingDown,
}

pub fn init_readiness() -> broadcast::Receiver<ReadinessEvent> {
    let sender = READINESS_SENDER.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        tx
    });
    sender.subscribe()
}

pub fn get_readiness_receiver() -> broadcast::Receiver<ReadinessEvent> {
    READINESS_SENDER
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(16);
            tx
        })
        .subscribe()
}

pub fn signal_ready() {
    API_READY.store(true, Ordering::SeqCst);
    if let Some(sender) = READINESS_SENDER.get() {
        if sender.send(ReadinessEvent::ApiReady).is_err() {
            tracing::debug!("No readiness receivers subscribed");
        }
    }
}

pub fn signal_shutdown() {
    API_READY.store(false, Ordering::SeqCst);
    if let Some(sender) = READINESS_SENDER.get() {
        if sender.send(ReadinessEvent::ApiShuttingDown).is_err() {
            tracing::debug!("No readiness receivers subscribed");
        }
    }
}

pub fn is_ready() -> bool {
    API_READY.load(Ordering::SeqCst)
}

pub async fn wait_for_ready(timeout_secs: u64) -> bool {
    if is_ready() {
        return true;
    }

    let mut receiver = get_readiness_receiver();

    tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
        while let Ok(event) = receiver.recv().await {
            if matches!(event, ReadinessEvent::ApiReady) {
                return true;
            }
        }
        false
    })
    .await
    .map_err(|_| {
        tracing::debug!(timeout_secs = timeout_secs, "Readiness wait timed out");
    })
    .unwrap_or(false)
}
