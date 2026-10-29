//! [`LiveCursor`]: what a live channel has delivered of an edit session's
//! update log, and the SSE id that resumes it.
//!
//! Updates reach a channel in publish order, which is not always sequence
//! order: two replicas log consecutive updates and relay them independently.
//! The cursor tracks the highest sequence number below which every update
//! was delivered and names that, not the update's own number, as each
//! update's resume id, so a client resuming after an out-of-order update
//! replays the one still missing. Updates a resumed channel already
//! replayed from the log are skipped when the topic delivers them again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_events::TopicEvent;
use systemprompt_identifiers::DocumentEditSessionId;
use systemprompt_models::documents::DocumentCollabUpdateEvent;

use super::{LiveOpen, LiveReplay, ResumeToken};

const AHEAD_MAX: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveDelivery {
    Skip,
    Deliver(Option<String>),
}

#[derive(Debug, Clone, Default)]
pub struct LiveCursor {
    session: Option<DocumentEditSessionId>,
    replayed_through: i64,
    through: i64,
    ahead: BTreeSet<i64>,
}

impl LiveCursor {
    #[must_use]
    pub fn start(
        open: &LiveOpen,
        resume: Option<&ResumeToken>,
        replay: Option<&LiveReplay>,
    ) -> Self {
        match (resume, replay) {
            (Some(token), Some(LiveReplay::Updates(_))) => Self::resumed(token),
            _ => Self::at(open.edit_session.clone(), open.seq.unwrap_or(0)),
        }
    }

    #[must_use]
    pub const fn at(session: Option<DocumentEditSessionId>, through: i64) -> Self {
        Self {
            session,
            replayed_through: 0,
            through,
            ahead: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn resumed(token: &ResumeToken) -> Self {
        Self {
            session: Some(token.session.clone()),
            replayed_through: token.seq,
            through: token.seq,
            ahead: BTreeSet::new(),
        }
    }

    pub fn replayed(&mut self, session: &DocumentEditSessionId, seq: i64) -> String {
        let id = self.advance(session, seq);
        self.replayed_through = self.replayed_through.max(seq);
        id
    }

    pub fn live(&mut self, event: &TopicEvent) -> LiveDelivery {
        if event.name != DocumentCollabUpdateEvent::NAME {
            return LiveDelivery::Deliver(None);
        }
        let Some(token) = event.resume_id.as_deref().and_then(ResumeToken::parse) else {
            return LiveDelivery::Deliver(None);
        };
        if self.session.as_ref() == Some(&token.session) && token.seq <= self.replayed_through {
            return LiveDelivery::Skip;
        }
        LiveDelivery::Deliver(Some(self.advance(&token.session, token.seq)))
    }

    #[must_use]
    pub const fn through(&self) -> i64 {
        self.through
    }

    fn advance(&mut self, session: &DocumentEditSessionId, seq: i64) -> String {
        if self.session.as_ref() != Some(session) {
            self.session = Some(session.clone());
            self.replayed_through = 0;
            self.through = 0;
            self.ahead.clear();
        }
        if seq > self.through {
            if self.ahead.len() >= AHEAD_MAX {
                self.ahead.clear();
            }
            self.ahead.insert(seq);
            while self.ahead.remove(&(self.through + 1)) {
                self.through += 1;
            }
        }
        format!("{session}:{}", self.through)
    }
}
