//! Group commit over the journal's active segment.
//!
//! A writer queues its frame and waits. The first waiter that finds no flush
//! in progress becomes the leader: it takes every queued frame, appends them
//! in one write, syncs the segment once, applies them to the in-memory index
//! and then publishes one outcome to every waiter of that batch. Nobody is
//! answered before the batch is synced, and a failed write or sync answers
//! every waiter in the batch with the same error and moves the next batch to
//! a fresh segment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use chacha20poly1305::ChaCha20Poly1305;

use super::index::{Op, State, apply};
use super::segment;
use crate::error::{GatewayAuditError, GatewayAuditResult as Result};

pub const JOURNAL_FSYNCS_TOTAL: &str = "gateway_journal_fsyncs_total";
pub const JOURNAL_RECORDS_TOTAL: &str = "gateway_journal_records_total";

/// How the journal makes a written segment and its directory durable.
///
/// [`Fsync`] is the production implementation. The journal is handed one
/// at open so a host can observe or replace the durability step; every
/// acknowledged write has passed through it.
pub trait JournalSync: Send + Sync + std::fmt::Debug {
    fn sync(&self, file: &File) -> std::io::Result<()>;
}

/// Durability by `fsync`: data and metadata reach stable storage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fsync;

impl JournalSync for Fsync {
    fn sync(&self, file: &File) -> std::io::Result<()> {
        file.sync_all()
    }
}

#[derive(Debug, Default)]
struct Batch {
    outcome: OnceLock<std::result::Result<(), Arc<std::io::Error>>>,
}

#[derive(Debug, Default)]
struct Queue {
    frames: Vec<(Vec<u8>, Op)>,
    batch: Arc<Batch>,
    flushing: bool,
}

#[derive(Debug)]
pub(super) struct Writer {
    pub seq: u64,
    next: u64,
    file: File,
    len: u64,
    poisoned: bool,
}

pub(super) struct Log {
    pub root: PathBuf,
    pub owner: String,
    pub cipher: ChaCha20Poly1305,
    pub sync: Arc<dyn JournalSync>,
    pub state: Mutex<State>,
    pub writer: Mutex<Writer>,
    queue: Mutex<Queue>,
    turn: Condvar,
    _owner_lock: File,
}

pub(super) fn guard<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_poisoned| GatewayAuditError::Invariant("Gateway journal state poisoned"))
}

impl Log {
    pub(super) fn create(
        root: PathBuf,
        cipher: ChaCha20Poly1305,
        sync: Arc<dyn JournalSync>,
    ) -> Result<Self> {
        let owner = uuid::Uuid::new_v4().simple().to_string();
        let owner_lock = segment::create(&segment::owner_path(&root, &owner))?;
        owner_lock.try_lock()?;
        let file = segment::create(&segment::segment_path(&root, &owner, 1))?;
        sync.sync(&File::open(&root)?)?;
        let mut state = State::default();
        state.live.insert(1, 0);
        Ok(Self {
            root,
            owner,
            cipher,
            sync,
            state: Mutex::new(state),
            writer: Mutex::new(Writer {
                seq: 1,
                next: 2,
                file,
                len: 0,
                poisoned: false,
            }),
            queue: Mutex::new(Queue::default()),
            turn: Condvar::new(),
            _owner_lock: owner_lock,
        })
    }

    pub(super) fn commit(&self, frame: Vec<u8>, op: Op) -> Result<()> {
        let batch = self.enqueue(frame, op)?;
        loop {
            if let Some(outcome) = batch.outcome.get() {
                return outcome.clone().map_err(GatewayAuditError::JournalFlush);
            }
            self.lead_or_wait(&batch)?;
        }
    }

    fn enqueue(&self, frame: Vec<u8>, op: Op) -> Result<Arc<Batch>> {
        let mut queue = guard(&self.queue)?;
        queue.frames.push((frame, op));
        Ok(Arc::clone(&queue.batch))
    }

    fn lead_or_wait(&self, batch: &Batch) -> Result<()> {
        let mut queue = guard(&self.queue)?;
        while queue.flushing && batch.outcome.get().is_none() {
            queue = self.turn.wait(queue).map_err(|_poisoned| {
                GatewayAuditError::Invariant("Gateway journal state poisoned")
            })?;
        }
        if batch.outcome.get().is_some() {
            return Ok(());
        }
        queue.flushing = true;
        let frames = std::mem::take(&mut queue.frames);
        let led = std::mem::take(&mut queue.batch);
        drop(queue);
        let _first = led.outcome.set(self.flush(frames));
        guard(&self.queue)?.flushing = false;
        self.turn.notify_all();
        Ok(())
    }

    fn flush(&self, frames: Vec<(Vec<u8>, Op)>) -> std::result::Result<(), Arc<std::io::Error>> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_poisoned| Arc::new(poisoned()))?;
        if writer.poisoned || writer.len >= segment::ROLL_BYTES {
            self.roll(&mut writer).map_err(Arc::new)?;
        }
        let mut buffer = Vec::new();
        let mut placed = Vec::with_capacity(frames.len());
        for (frame, op) in frames {
            let offset = writer.len + buffer.len() as u64 + 4;
            let len = u32::try_from(frame.len() - 4).unwrap_or(u32::MAX);
            buffer.extend_from_slice(&frame);
            placed.push((op, offset, len));
        }
        let written = writer
            .file
            .write_all(&buffer)
            .and_then(|()| self.sync.sync(&writer.file));
        metrics::counter!(JOURNAL_FSYNCS_TOTAL).increment(1);
        if let Err(error) = written {
            writer.poisoned = true;
            return Err(Arc::new(error));
        }
        writer.len += buffer.len() as u64;
        metrics::counter!(JOURNAL_RECORDS_TOTAL).increment(placed.len() as u64);
        let seq = writer.seq;
        drop(writer);
        let mut state = self
            .state
            .lock()
            .map_err(|_poisoned| Arc::new(poisoned()))?;
        for (op, offset, len) in placed {
            apply(&mut state, op, seq, offset, len);
        }
        Ok(())
    }

    fn roll(&self, writer: &mut Writer) -> std::io::Result<()> {
        let seq = writer.next;
        writer.next += 1;
        let file = segment::create(&segment::segment_path(&self.root, &self.owner, seq))?;
        self.sync.sync(&File::open(&self.root)?)?;
        metrics::counter!(JOURNAL_FSYNCS_TOTAL).increment(1);
        *writer = Writer {
            seq,
            next: seq + 1,
            file,
            len: 0,
            poisoned: false,
        };
        self.state
            .lock()
            .map_err(|_poisoned| poisoned())?
            .live
            .insert(seq, 0);
        Ok(())
    }
}

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Log")
            .field("root", &self.root)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

fn poisoned() -> std::io::Error {
    std::io::Error::other("gateway journal state poisoned")
}
