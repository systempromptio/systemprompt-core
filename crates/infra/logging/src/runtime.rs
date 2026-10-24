//! Owned handles to the logging pipeline.
//!
//! [`LogSink`] is what [`crate::init_logging`] and the console initialisers
//! return: a handle onto the database layer inside the installed `tracing`
//! subscriber. It enqueues entries persisted outside a `tracing` call, reports
//! the drop count, and flushes the writer on shutdown. [`LoggingRuntime`]
//! pairs a sink with the resolved platform owner, so originator-less telemetry
//! is attributed without a process-wide cell. The runtime builds it once at
//! bootstrap and stores it on `AppContext`; code with no context in scope is
//! handed the value through its constructor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{InstanceId, TraceId};
use systemprompt_manifest::services::SystemAdmin;

use crate::layer::{LogWriterShutdownError, ProxyDatabaseLayer};
use crate::models::{LogActor, LogEntry};

/// Handle onto the database sink of the installed subscriber; clones share it.
#[derive(Debug, Clone, Default)]
pub struct LogSink {
    proxy: ProxyDatabaseLayer,
}

impl LogSink {
    pub(crate) const fn from_proxy(proxy: ProxyDatabaseLayer) -> Self {
        Self { proxy }
    }

    pub(crate) fn attach(&self, db_pool: &DbPool, instance_id: Option<InstanceId>) {
        self.proxy.attach_owned(db_pool, instance_id);
    }

    pub fn enqueue(&self, entry: LogEntry) {
        self.proxy.enqueue(entry);
    }

    #[must_use]
    pub fn dropped_entries(&self) -> u64 {
        self.proxy.shared().dropped()
    }

    #[must_use]
    pub fn instance_id(&self) -> Option<&InstanceId> {
        self.proxy.shared().instance.get()
    }

    pub async fn shutdown(&self) -> Result<(), LogWriterShutdownError> {
        self.proxy.shutdown().await
    }
}

/// The process's logging handle: the sink plus the platform owner.
#[derive(Debug, Clone)]
pub struct LoggingRuntime {
    sink: LogSink,
    platform_owner: Arc<SystemAdmin>,
}

impl LoggingRuntime {
    #[must_use]
    pub const fn new(sink: LogSink, platform_owner: Arc<SystemAdmin>) -> Self {
        Self {
            sink,
            platform_owner,
        }
    }

    #[must_use]
    pub const fn sink(&self) -> &LogSink {
        &self.sink
    }

    #[must_use]
    pub fn platform_owner(&self) -> &SystemAdmin {
        &self.platform_owner
    }

    #[must_use]
    pub fn platform_actor(&self, trace_id: TraceId) -> LogActor {
        LogActor::platform(&self.platform_owner, trace_id)
    }

    #[must_use]
    pub fn instance_id(&self) -> Option<&InstanceId> {
        self.sink.instance_id()
    }

    pub fn enqueue(&self, entry: LogEntry) {
        self.sink.enqueue(entry);
    }
}
