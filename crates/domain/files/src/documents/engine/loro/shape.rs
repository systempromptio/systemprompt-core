//! The header check every update passes before any import: its size, its
//! mode, its change, peer and operation counts, the consistency of its
//! version ranges and, for a client update, its peers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{EncodedBlobMode, LoroDoc, PeerID};

/// Why an update was refused from its encoded header, before any import.
#[derive(Debug, thiserror::Error)]
pub enum UpdateShapeError {
    #[error("the update is {len} bytes, over the {max} byte bound")]
    TooLarge { len: usize, max: usize },
    #[error("the update header could not be decoded")]
    Header(#[source] loro::LoroError),
    #[error("the blob is a {mode}, not an incremental update")]
    NotAnUpdate { mode: EncodedBlobMode },
    #[error("the update carries {changes} changes, over the {max} change bound")]
    TooManyChanges { changes: u32, max: u32 },
    #[error("the update spans {peers} peers, over the {max} peer bound")]
    TooManyPeers { peers: usize, max: usize },
    #[error("the update's version range for peer {peer} is inconsistent")]
    InconsistentRange { peer: PeerID },
    #[error("the update spans {ops} operations of peer {peer}, over the {max} bound")]
    RangeTooWide { peer: PeerID, ops: i64, max: i64 },
    #[error("the update carries operations of peer {peer}, which the caller was not assigned")]
    PeerNotAssigned { peer: PeerID },
}

pub const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_UPDATE_CHANGES: u32 = 50_000;
pub const MAX_UPDATE_PEERS: usize = 64;
pub const MAX_UPDATE_PEER_OPS: i64 = 1_000_000;

pub fn check_update_shape(update: &[u8], peers: Option<&[u64]>) -> Result<(), UpdateShapeError> {
    if update.len() > MAX_UPDATE_BYTES {
        return Err(UpdateShapeError::TooLarge {
            len: update.len(),
            max: MAX_UPDATE_BYTES,
        });
    }
    let meta = LoroDoc::decode_import_blob_meta(update, true).map_err(UpdateShapeError::Header)?;
    if !matches!(meta.mode, EncodedBlobMode::Updates) {
        return Err(UpdateShapeError::NotAnUpdate { mode: meta.mode });
    }
    if meta.change_num > MAX_UPDATE_CHANGES {
        return Err(UpdateShapeError::TooManyChanges {
            changes: meta.change_num,
            max: MAX_UPDATE_CHANGES,
        });
    }
    let (start, end) = (&meta.partial_start_vv, &meta.partial_end_vv);
    if end.len() > MAX_UPDATE_PEERS || start.len() > MAX_UPDATE_PEERS {
        return Err(UpdateShapeError::TooManyPeers {
            peers: end.len().max(start.len()),
            max: MAX_UPDATE_PEERS,
        });
    }
    if let Some(peer) = start.keys().find(|peer| !end.contains_key(*peer)) {
        return Err(UpdateShapeError::InconsistentRange { peer: *peer });
    }
    for (peer, last) in end.iter() {
        let first = start.get(peer).copied().unwrap_or(0);
        if first < 0 || *last < first {
            return Err(UpdateShapeError::InconsistentRange { peer: *peer });
        }
        let ops = i64::from(*last) - i64::from(first);
        if ops > MAX_UPDATE_PEER_OPS {
            return Err(UpdateShapeError::RangeTooWide {
                peer: *peer,
                ops,
                max: MAX_UPDATE_PEER_OPS,
            });
        }
        if ops > 0
            && let Some(assigned) = peers
            && !assigned.contains(peer)
        {
            return Err(UpdateShapeError::PeerNotAssigned { peer: *peer });
        }
    }
    Ok(())
}
