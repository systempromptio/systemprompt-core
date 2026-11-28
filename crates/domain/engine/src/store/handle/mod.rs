//! The store handle: one writer connection behind a mutex, one read connection
//! for file databases, and the change feed. Writes run in an immediate
//! transaction; their changes are published only after the commit succeeds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod fork;
mod messages;
mod parts;
mod projects;
mod sessions;

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use tokio::sync::broadcast;

use crate::store::change::Change;
use crate::store::error::{Result, StoreError};
use crate::store::id::IdClock;
use crate::store::schema;

const CHANGE_CAPACITY: usize = 1024;

/// A cheaply cloneable handle on the session database.
///
/// Every method is synchronous and finishes quickly; call it from
/// `spawn_blocking` on an async runtime. Writes are serialised on one
/// connection; reads of a file database use a second connection so they never
/// wait behind a write.
///
/// [`Store::subscribe`] yields every change committed after the call. A
/// receiver that falls more than the channel capacity behind gets
/// `RecvError::Lagged` and should reload what it shows.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    writer: Mutex<Connection>,
    reader: Option<Mutex<Connection>>,
    changes: broadcast::Sender<Change>,
    location: String,
    ids: IdClock,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("location", &self.inner.location)
            .finish_non_exhaustive()
    }
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let open = |path: &Path| {
            Connection::open(path).map_err(|source| StoreError::Open {
                path: path.to_path_buf(),
                source,
            })
        };
        create_owner_only(path)?;
        let mut writer = open(path)?;
        schema::configure_writer(&writer)?;
        let version = schema::migrate(&mut writer)?;
        let reader = open(path)?;
        schema::configure_reader(&reader)?;
        tracing::debug!(path = %path.display(), version, "store opened");
        Ok(Self::assemble(
            writer,
            Some(reader),
            path.display().to_string(),
        ))
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut writer = Connection::open_in_memory()?;
        schema::configure_writer(&writer)?;
        schema::migrate(&mut writer)?;
        Ok(Self::assemble(writer, None, String::from(":memory:")))
    }

    fn assemble(writer: Connection, reader: Option<Connection>, location: String) -> Self {
        let (changes, _) = broadcast::channel(CHANGE_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                writer: Mutex::new(writer),
                reader: reader.map(Mutex::new),
                changes,
                location,
                ids: IdClock::new(),
            }),
        }
    }

    pub fn ids(&self) -> &IdClock {
        &self.inner.ids
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.inner.changes.subscribe()
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.read(|connection| schema::version(connection))
    }

    pub(crate) fn read<T>(&self, run: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        if let Some(reader) = &self.inner.reader {
            let mut connection = reader.lock();
            run(&mut connection)
        } else {
            let mut connection = self.inner.writer.lock();
            run(&mut connection)
        }
    }

    pub(crate) fn write<T>(
        &self,
        run: impl FnOnce(&Transaction<'_>, &mut Vec<Change>) -> Result<T>,
    ) -> Result<T> {
        let mut changes = Vec::new();
        let mut connection = self.inner.writer.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = run(&transaction, &mut changes)?;
        transaction.commit()?;
        drop(connection);
        for change in changes {
            if self.inner.changes.send(change).is_err() {
                tracing::trace!("change committed with no subscriber");
            }
        }
        Ok(value)
    }
}

fn create_owner_only(path: &Path) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    crate::store::platform::owner_only(&mut options);
    options
        .open(path)
        .map(drop)
        .map_err(|source| StoreError::CreateFile {
            path: path.to_path_buf(),
            source,
        })
}
