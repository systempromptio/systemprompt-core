//! One server's background job: connect, list tools, publish them, then follow
//! tool list changes until the server goes away or the job is cancelled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::config::{ServerSpec, ServerStatus};
use super::state::{ConnectSettings, Shared, Slot};
use crate::client::{McpClient, RequestOptions, ServerNotification};
use crate::error::{McpError, describe};
use crate::headers::HeaderCell;
use crate::transport::LocalCommand;

const TOOLS_CHANGED: &str = "notifications/tools/list_changed";

pub(super) struct ServerJob {
    pub(super) shared: Weak<Shared>,
    pub(super) settings: Arc<ConnectSettings>,
    pub(super) name: String,
    pub(super) epoch: u64,
    pub(super) spec: ServerSpec,
    pub(super) headers: HeaderCell,
    pub(super) timeout: Duration,
    pub(super) cancel: CancellationToken,
}

impl ServerJob {
    pub(super) async fn run(self) {
        let Some(client) = self.connect_in_time().await else {
            return;
        };
        let notes = client.notifications();
        if self.publish_first_list(&client).await {
            self.follow(&client, notes).await;
        }
    }

    async fn connect_in_time(&self) -> Option<McpClient> {
        let connecting = tokio::time::timeout(self.timeout, self.connect());
        let outcome = tokio::select! {
            () = self.cancel.cancelled() => return None,
            outcome = connecting => outcome,
        };
        match outcome {
            Ok(Ok(client)) => Some(client),
            Ok(Err(error)) => {
                self.fail(&error);
                None
            },
            Err(_elapsed) => {
                self.fail(&McpError::ConnectTimeout(self.timeout));
                None
            },
        }
    }

    async fn publish_first_list(&self, client: &McpClient) -> bool {
        let listed = tokio::select! {
            () = self.cancel.cancelled() => {
                client.shutdown().await;
                return false;
            }
            listed = client.list_tools(self.list_options()) => listed,
        };
        let tools = match listed {
            Ok(tools) => tools,
            Err(error) => {
                client.shutdown().await;
                self.fail(&error);
                return false;
            },
        };
        let count = tools.len();
        let accepted = self.update(|slot| {
            slot.client = Some(client.clone());
            slot.tools = tools;
            slot.status = ServerStatus::Connected { tools: count };
        });
        if !accepted {
            client.shutdown().await;
            return false;
        }
        tracing::info!(server = %self.name, tools = count, "server connected");
        true
    }

    async fn follow(&self, client: &McpClient, mut notes: broadcast::Receiver<ServerNotification>) {
        loop {
            tokio::select! {
                () = self.cancel.cancelled() => {
                    client.shutdown().await;
                    return;
                }
                () = client.closed() => {
                    self.lost();
                    return;
                }
                note = notes.recv() => match note {
                    Ok(note) if note.method == TOOLS_CHANGED => self.refresh(client).await,
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => self.refresh(client).await,
                    Err(broadcast::error::RecvError::Closed) => {
                        self.lost();
                        return;
                    }
                },
            }
        }
    }

    fn list_options(&self) -> RequestOptions {
        RequestOptions {
            timeout: Some(self.settings.request_timeout),
            cancel: Some(self.cancel.clone()),
        }
    }

    async fn connect(&self) -> Result<McpClient, McpError> {
        match &self.spec {
            ServerSpec::Remote { url, .. } => self.connect_remote(url).await,
            ServerSpec::Local {
                command,
                args,
                env,
                cwd,
            } => {
                let local = LocalCommand {
                    command,
                    args,
                    env,
                    cwd: cwd.as_deref(),
                };
                let client = McpClient::stdio(&self.name, local)?;
                initialized(client, &self.settings).await
            },
        }
    }

    async fn connect_remote(&self, url: &str) -> Result<McpClient, McpError> {
        let settings = &*self.settings;
        let url = Url::parse(url).map_err(McpError::Url)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(McpError::Protocol(
                "a remote server URL must be http or https",
            ));
        }
        let headers = Arc::clone(&self.headers);
        let client = McpClient::streamable_http(settings.http.clone(), url.clone(), headers);
        match client
            .initialize(&settings.client_info, Some(settings.request_timeout))
            .await
        {
            Ok(_) => Ok(client),
            Err(McpError::Status { status }) if falls_back(status) => {
                client.shutdown().await;
                tracing::debug!(server = %self.name, status, "streamable HTTP refused; trying legacy SSE");
                let headers = Arc::clone(&self.headers);
                let legacy = McpClient::legacy_sse(settings.http.clone(), url, headers).await?;
                initialized(legacy, settings).await
            },
            Err(error) => {
                client.shutdown().await;
                Err(error)
            },
        }
    }

    async fn refresh(&self, client: &McpClient) {
        match client.list_tools(self.list_options()).await {
            Ok(tools) => {
                let count = tools.len();
                self.update(|slot| {
                    slot.tools = tools;
                    slot.status = ServerStatus::Connected { tools: count };
                });
                tracing::debug!(server = %self.name, tools = count, "tool list refreshed");
            },
            Err(error) => {
                tracing::debug!(server = %self.name, error = %describe(&error), "tool list refresh failed");
            },
        }
    }

    fn fail(&self, error: &McpError) {
        let status = match error {
            McpError::Unauthorized => {
                tracing::info!(server = %self.name, "server needs authorization");
                ServerStatus::NeedsAuth
            },
            other => {
                tracing::warn!(server = %self.name, error = %describe(other), "server failed to connect");
                ServerStatus::Failed {
                    error: describe(other),
                }
            },
        };
        self.update(|slot| {
            slot.client = None;
            slot.tools.clear();
            slot.status = status;
        });
    }

    fn lost(&self) {
        tracing::warn!(server = %self.name, "server connection closed");
        self.update(|slot| {
            slot.client = None;
            slot.tools.clear();
            slot.status = ServerStatus::Failed {
                error: McpError::Closed.to_string(),
            };
        });
    }

    fn update(&self, change: impl FnOnce(&mut Slot)) -> bool {
        self.shared
            .upgrade()
            .is_some_and(|shared| shared.update_slot(&self.name, self.epoch, change))
    }
}

fn falls_back(status: u16) -> bool {
    (400..500).contains(&status) && status != 401
}

async fn initialized(client: McpClient, settings: &ConnectSettings) -> Result<McpClient, McpError> {
    match client
        .initialize(&settings.client_info, Some(settings.request_timeout))
        .await
    {
        Ok(_) => Ok(client),
        Err(error) => {
            client.shutdown().await;
            Err(error)
        },
    }
}
