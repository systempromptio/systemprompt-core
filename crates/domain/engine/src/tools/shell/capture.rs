//! What a running shell command's output turns into: a bounded capture that
//! keeps the head and the tail when the middle has to go, and a throttled live
//! tail for the progress sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::time::Instant;

use crate::contract::Progress;
use crate::text::{ceil_boundary, floor_boundary};

const LIVE_TAIL_BYTES: usize = 8 * 1024;
const LIVE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(super) struct Capture {
    head: String,
    tail: String,
    limit: usize,
    omitted: usize,
    overflowed: bool,
}

impl Capture {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            head: String::new(),
            tail: String::new(),
            limit: limit.max(1024),
            omitted: 0,
            overflowed: false,
        }
    }

    pub(super) fn push(&mut self, chunk: &str) {
        if !self.overflowed {
            if self.head.len() + chunk.len() <= self.limit {
                self.head.push_str(chunk);
                return;
            }
            self.overflowed = true;
        }
        self.tail.push_str(chunk);
        let keep = self.limit / 2;
        if self.tail.len() > keep * 2 {
            let cut = ceil_boundary(&self.tail, self.tail.len() - keep);
            self.omitted += cut;
            self.tail.drain(..cut);
        }
    }

    pub(super) fn finish(self) -> String {
        if self.omitted == 0 {
            return format!("{}{}", self.head, self.tail);
        }
        format!(
            "{}\n[… {} bytes of output omitted …]\n{}",
            self.head, self.omitted, self.tail
        )
    }
}

pub(super) struct Live {
    progress: Option<Arc<dyn Progress>>,
    title: String,
    tail: String,
    last: Option<Instant>,
    dirty: bool,
}

impl Live {
    pub(super) const fn new(progress: Option<Arc<dyn Progress>>, title: String) -> Self {
        Self {
            progress,
            title,
            tail: String::new(),
            last: None,
            dirty: false,
        }
    }

    pub(super) fn push(&mut self, chunk: &str) {
        if self.progress.is_none() {
            return;
        }
        self.tail.push_str(chunk);
        if self.tail.len() > LIVE_TAIL_BYTES * 2 {
            let cut = floor_boundary(&self.tail, self.tail.len() - LIVE_TAIL_BYTES);
            let cut = ceil_boundary(&self.tail, cut);
            self.tail.drain(..cut);
        }
        self.dirty = true;
        if self.last.is_none_or(|last| last.elapsed() >= LIVE_INTERVAL) {
            self.flush();
        }
    }

    pub(super) fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        if let Some(progress) = &self.progress {
            progress.update(&self.title, json!({ "output": self.tail }));
        }
        self.last = Some(Instant::now());
        self.dirty = false;
    }
}
