use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::{EventHandler, McpEvent};

#[derive(Debug)]
pub struct HealthCheckHandler {
    max_failures: usize,
    failure_counts: Arc<RwLock<HashMap<String, usize>>>,
    restart_sender: Option<broadcast::Sender<McpEvent>>,
}

impl Default for HealthCheckHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckHandler {
    pub fn new() -> Self {
        Self {
            max_failures: 3,
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
            restart_sender: None,
        }
    }

    pub fn with_restart_sender(mut self, sender: broadcast::Sender<McpEvent>) -> Self {
        self.restart_sender = Some(sender);
        self
    }
}

#[async_trait]
impl EventHandler for HealthCheckHandler {
    async fn handle(&self, event: &McpEvent) -> Result<()> {
        match event {
            McpEvent::HealthCheckFailed {
                service_name,
                reason,
            } => {
                let mut failures = self.failure_counts.write().await;
                let count = failures.entry(service_name.clone()).or_insert(0);
                *count += 1;

                tracing::warn!(
                    service = %service_name,
                    failures = *count,
                    reason = %reason,
                    "Health check failed"
                );

                let should_restart = *count >= self.max_failures;
                let failure_count = *count;
                drop(failures);

                if should_restart {
                    tracing::error!(
                        service = %service_name,
                        max_failures = self.max_failures,
                        "Service exceeded max failures, requesting restart"
                    );

                    if let Some(sender) = &self.restart_sender {
                        let restart_event = McpEvent::ServiceRestartRequested {
                            service_name: service_name.clone(),
                            reason: format!("Health check failed {} times", failure_count),
                        };
                        let _ = sender.send(restart_event);
                    }
                }
            },
            McpEvent::ServiceStarted { service_name, .. } => {
                let mut failures = self.failure_counts.write().await;
                failures.remove(service_name);
            },
            _ => {},
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "health_check"
    }

    fn handles(&self, event: &McpEvent) -> bool {
        matches!(
            event,
            McpEvent::HealthCheckFailed { .. } | McpEvent::ServiceStarted { .. }
        )
    }
}
