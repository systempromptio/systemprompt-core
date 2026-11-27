//! Engine bookkeeping kept in a session's metadata: which user messages were
//! steering messages, and where the latest compaction's verbatim tail begins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use crate::store::{MessageId, Session, SessionId, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::run::db::Db;
use crate::run::error::EngineError;

const STEERED: &str = "steered";
const COMPACTION: &str = "compaction";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CompactionMark {
    pub(crate) message: MessageId,
    #[serde(default)]
    pub(crate) tail: Option<MessageId>,
}

pub(crate) fn steered(session: &Session) -> HashSet<MessageId> {
    session
        .metadata
        .get(STEERED)
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .filter_map(|id| MessageId::parse(id).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn compaction(session: &Session) -> Option<CompactionMark> {
    let value = session.metadata.get(COMPACTION)?;
    match serde_json::from_value(value.clone()) {
        Ok(mark) => Some(mark),
        Err(error) => {
            tracing::warn!(session = %session.id, error = %error, "compaction mark is unreadable");
            None
        },
    }
}

pub(crate) async fn mark_steered(
    db: &Db,
    session: &SessionId,
    message: &MessageId,
) -> Result<(), EngineError> {
    let key = session.clone();
    let id = Value::String(message.to_string());
    db.run(move |store| {
        store.update_session(&key, |session| {
            let list = session
                .metadata
                .entry(STEERED)
                .or_insert_with(|| Value::Array(Vec::new()));
            match list {
                Value::Array(ids) => ids.push(id),
                other => *other = Value::Array(vec![id]),
            }
        })
    })
    .await?;
    Ok(())
}

pub(crate) async fn set_compaction(
    db: &Db,
    session: &SessionId,
    mark: &CompactionMark,
) -> Result<(), EngineError> {
    let key = session.clone();
    let value = serde_json::to_value(mark).map_err(StoreError::from)?;
    db.run(move |store| {
        store.update_session(&key, |session| {
            session.metadata.insert(COMPACTION.to_owned(), value);
        })
    })
    .await?;
    Ok(())
}
