//! Keeping the server's copy of touched documents in step with the disk, and
//! waiting for the diagnostics a change produces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::atomic::Ordering;

use serde_json::json;
use tokio::time::Instant;
use tracing::debug;

use super::Client;
use crate::lsp::error::LspError;
use crate::lsp::language::language_id;
use crate::lsp::parse::SaveMode;
use crate::lsp::types::Diagnostic;
use crate::lsp::uri;

#[derive(Debug)]
pub(super) struct Document {
    version: i32,
    text: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Synced {
    pub(crate) version: i32,
    pub(crate) changed: bool,
    pub(crate) since: u64,
}

impl Client {
    pub(crate) fn sync_document(&self, path: &Path, text: &str) -> Result<Synced, LspError> {
        let uri = uri::path_to_uri(path);
        let since = self.shared.publish_seq.load(Ordering::SeqCst);
        let (version, changed) = self.update_document(path, &uri, text)?;
        if changed {
            self.notify_saved(&uri, text)?;
        }
        Ok(Synced {
            version,
            changed,
            since,
        })
    }

    fn update_document(&self, path: &Path, uri: &str, text: &str) -> Result<(i32, bool), LspError> {
        let mut documents = self.documents.lock();
        let synced = match documents.get_mut(path) {
            Some(document) if document.text == text => (document.version, false),
            Some(document) => {
                document.version = document.version.wrapping_add(1);
                text.clone_into(&mut document.text);
                self.shared.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": document.version },
                        "contentChanges": [{ "text": text }]
                    }),
                )?;
                (document.version, true)
            },
            None => {
                self.shared.notify(
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": language_id(path),
                            "version": 1,
                            "text": text
                        }
                    }),
                )?;
                documents.insert(
                    path.to_path_buf(),
                    Document {
                        version: 1,
                        text: text.to_owned(),
                    },
                );
                (1, true)
            },
        };
        drop(documents);
        Ok(synced)
    }

    fn notify_saved(&self, uri: &str, text: &str) -> Result<(), LspError> {
        match self.capabilities.save {
            Some(SaveMode::Plain) => self.shared.notify(
                "textDocument/didSave",
                json!({ "textDocument": { "uri": uri } }),
            ),
            Some(SaveMode::WithText) => self.shared.notify(
                "textDocument/didSave",
                json!({ "textDocument": { "uri": uri }, "text": text }),
            ),
            None => Ok(()),
        }
    }

    pub(crate) fn close_document(&self, path: &Path) {
        let removed = self.documents.lock().remove(path).is_some();
        self.shared.published.lock().remove(path);
        if removed
            && let Err(error) = self.shared.notify(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": uri::path_to_uri(path) } }),
            )
        {
            debug!(server = %self.shared.server, %error, "could not close a document");
        }
    }

    pub(crate) fn has_published(&self, path: &Path) -> bool {
        self.shared.published.lock().contains_key(path)
    }

    fn fresh_publish(&self, path: &Path, since: u64, version: i32) -> Option<u64> {
        let (seq, seen) = self
            .shared
            .published
            .lock()
            .get(path)
            .map(|entry| (entry.seq, entry.version))?;
        if seq <= since {
            return None;
        }
        match seen {
            Some(seen) if seen != i64::from(version) => None,
            _ => Some(seq),
        }
    }

    // Why: servers often publish twice for one change (a fast syntax pass, then a
    // slower semantic one), so the wait ends only after the file's diagnostics
    // have been quiet for the debounce window, or at the cap.
    pub(crate) async fn wait_for_diagnostics(&self, path: &Path, synced: Synced) {
        let mut events = self.shared.publish_events.subscribe();
        let deadline = Instant::now() + self.timeouts.diagnostics_cap;
        let mut last_seq = None;
        let mut settle_at: Option<Instant> = None;
        loop {
            if !self.is_alive() {
                return;
            }
            if let Some(seq) = self.fresh_publish(path, synced.since, synced.version)
                && last_seq != Some(seq)
            {
                last_seq = Some(seq);
                settle_at = Some(Instant::now() + self.timeouts.diagnostics_debounce);
            }
            let wake = settle_at.map_or(deadline, |at| at.min(deadline));
            tokio::select! {
                () = tokio::time::sleep_until(wake) => return,
                changed = events.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
        }
    }

    pub(crate) fn diagnostics_for(&self, path: &Path) -> Vec<Diagnostic> {
        self.shared
            .published
            .lock()
            .get(path)
            .map(|entry| entry.diagnostics.clone())
            .unwrap_or_default()
    }

    pub(crate) fn all_diagnostics(&self) -> Vec<Diagnostic> {
        self.shared
            .published
            .lock()
            .values()
            .flat_map(|entry| entry.diagnostics.iter().cloned())
            .collect()
    }
}
