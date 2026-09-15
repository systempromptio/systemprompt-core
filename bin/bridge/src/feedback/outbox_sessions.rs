//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Delivery, FeedbackError, Outbox, Result, State};
use std::collections::BTreeMap;
use systemprompt_identifiers::InstallationReceiptId;

impl Outbox {
    pub fn queue_session(
        &self,
        host: systemprompt_models::feedback::EvaluatorClient,
        session: &str,
    ) -> Result<()> {
        if session.is_empty() || session.len() > 512 {
            return Err(FeedbackError::Scope);
        }
        let session_key = serde_json::to_string(&(host, session))?;
        if self.read_locked(|state| Ok(state.sessions.contains_key(&session_key)))? {
            return Ok(());
        }
        self.mutate(|state| {
            if state.sessions.contains_key(&session_key) {
                return Ok(());
            }
            compact_completed(state)?;
            if state.sessions.len() >= 1024 {
                let old = state
                    .completed_sessions
                    .iter()
                    .next()
                    .cloned()
                    .ok_or(FeedbackError::Full)?;
                state.completed_sessions.remove(&old);
                state.sessions.remove(&old);
            }
            let mut newest: BTreeMap<_, i64> = BTreeMap::new();
            for entry in state
                .entries
                .values()
                .filter(|entry| entry.request.host == host)
            {
                newest
                    .entry(entry.request.resource_id.clone())
                    .and_modify(|date| *date = (*date).max(entry.request.generation))
                    .or_insert(entry.request.generation);
            }
            for pending in state
                .pending_installations
                .values()
                .filter(|pending| pending.host == host && !pending.superseded)
            {
                newest
                    .entry(pending.publication.resource_id.clone())
                    .and_modify(|generation| {
                        *generation = (*generation).max(pending.publication.generation);
                    })
                    .or_insert(pending.publication.generation);
            }
            let mut publications = Vec::new();
            for pending in state
                .pending_installations
                .values()
                .filter(|pending| pending.host == host && !pending.superseded)
            {
                if newest.get(&pending.publication.resource_id)
                    == Some(&pending.publication.generation)
                {
                    publications.push(pending.publication.publication_id.clone());
                }
            }
            for entry in state
                .entries
                .values_mut()
                .filter(|entry| entry.request.host == host)
            {
                if newest.get(&entry.request.resource_id) != Some(&entry.request.generation) {
                    continue;
                }
                if entry
                    .session_bindings
                    .values()
                    .filter(|bound| !**bound)
                    .count()
                    >= 256
                    && !entry.session_bindings.contains_key(session)
                {
                    return Err(FeedbackError::Full);
                }
                publications.push(entry.request.publication_id.clone());
                entry
                    .session_bindings
                    .entry(session.to_owned())
                    .or_insert(false);
            }
            publications.sort();
            publications.dedup();
            state.sessions.insert(session_key, publications);
            compact_completed(state)?;
            Ok(())
        })
    }

    pub fn acknowledge_session(
        &self,
        key: &str,
        receipt: &InstallationReceiptId,
        session: &str,
    ) -> Result<()> {
        self.mutate(|state| {
            let entry = state.entries.get_mut(key).ok_or(FeedbackError::Scope)?;
            let verified = match &entry.delivery {
                Delivery::Acknowledged(response) => {
                    &response.receipt_id == receipt && response.fully_verified
                },
                _ => false,
            };
            if !verified {
                return Err(FeedbackError::Scope);
            }
            let session_key = serde_json::to_string(&(entry.request.host, session))?;
            if state.completed_sessions.contains(&session_key) {
                return Ok(());
            }
            let bound = entry
                .session_bindings
                .get_mut(session)
                .ok_or(FeedbackError::Scope)?;
            *bound = true;
            compact_completed(state)?;
            Ok(())
        })
    }
}

fn compact_completed(state: &mut State) -> Result<()> {
    for (key, publications) in &state.sessions {
        if state.completed_sessions.contains(key) {
            continue;
        }
        let (host, session): (systemprompt_models::feedback::EvaluatorClient, String) =
            serde_json::from_str(key)?;
        if publications.iter().all(|publication| {
            let entries: Vec<_> = state.entries.values().filter(|entry| {
                entry.request.host == host && &entry.request.publication_id == publication
            }).collect();
            !entries.is_empty() && entries.iter().all(|entry| {
                matches!(&entry.delivery, Delivery::Acknowledged(response) if response.fully_verified)
                    && entry.session_bindings.get(&session) == Some(&true)
            })
        }) {
            state.completed_sessions.insert(key.clone());
            for entry in state.entries.values_mut().filter(|entry| entry.request.host == host) {
                entry.session_bindings.remove(&session);
            }
        }
    }
    Ok(())
}
