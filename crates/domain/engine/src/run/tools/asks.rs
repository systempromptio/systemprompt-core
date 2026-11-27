//! Questions waiting for the user: permission requests and the question tool's
//! questions, each answered once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::Hash;

use crate::config::{Action, Rule, Ruleset};
use systemprompt_identifiers::engine::{PermissionId, QuestionId};
use crate::store::SessionId;
use harness_tasks::CancellationToken;
use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::run::events::{EngineEvent, PermissionAsk, QuestionAsk, Reply};
use crate::run::inner::Inner;

pub(crate) type Answers = Vec<Vec<String>>;

struct PendingPermission {
    ask: PermissionAsk,
    reply: oneshot::Sender<Reply>,
}

struct PendingQuestion {
    ask: QuestionAsk,
    reply: oneshot::Sender<Option<Answers>>,
}

#[derive(Default)]
struct State {
    permissions: HashMap<PermissionId, PendingPermission>,
    questions: HashMap<QuestionId, PendingQuestion>,
}

#[derive(Default)]
pub(crate) struct Asks {
    state: Mutex<State>,
}

impl Asks {
    pub(crate) async fn permission(
        &self,
        inner: &Inner,
        ask: PermissionAsk,
        cancel: &CancellationToken,
    ) -> Option<Reply> {
        let (sender, receiver) = oneshot::channel();
        let id = ask.id.clone();
        let session = ask.session.clone();
        let pending = PendingPermission {
            ask: ask.clone(),
            reply: sender,
        };
        self.state.lock().permissions.insert(id.clone(), pending);
        inner.emit(EngineEvent::PermissionAsked(Box::new(ask)));
        tokio::select! {
            () = cancel.cancelled() => {
                let removed = self.state.lock().permissions.remove(&id);
                if removed.is_some() {
                    let reply = Reply::Reject { feedback: None };
                    inner.emit(EngineEvent::PermissionResolved { id, session, reply });
                }
                None
            }
            reply = receiver => reply.ok(),
        }
    }

    pub(crate) fn reply_permission(&self, inner: &Inner, id: &PermissionId, reply: Reply) -> bool {
        let (pending, siblings) = {
            let mut state = self.state.lock();
            let Some(pending) = state.permissions.remove(id) else {
                return false;
            };
            let siblings = if reply == Reply::Always {
                covered(&mut state, &pending.ask)
            } else {
                Vec::new()
            };
            drop(state);
            (pending, siblings)
        };
        let session = pending.ask.session.clone();
        deliver(pending.reply, reply.clone(), id);
        inner.emit(EngineEvent::PermissionResolved {
            id: id.clone(),
            session,
            reply,
        });
        for sibling in siblings {
            deliver(sibling.reply, Reply::Once, &sibling.ask.id);
            let (id, session) = (sibling.ask.id, sibling.ask.session);
            inner.emit(EngineEvent::PermissionResolved {
                id,
                session,
                reply: Reply::Once,
            });
        }
        true
    }

    pub(crate) async fn question(
        &self,
        inner: &Inner,
        ask: QuestionAsk,
        cancel: &CancellationToken,
    ) -> Option<Option<Answers>> {
        let (sender, receiver) = oneshot::channel();
        let id = ask.id.clone();
        let session = ask.session.clone();
        let pending = PendingQuestion {
            ask: ask.clone(),
            reply: sender,
        };
        self.state.lock().questions.insert(id.clone(), pending);
        inner.emit(EngineEvent::QuestionAsked(ask));
        tokio::select! {
            () = cancel.cancelled() => {
                let removed = self.state.lock().questions.remove(&id);
                if removed.is_some() {
                    inner.emit(EngineEvent::QuestionResolved { id, session, answered: false });
                }
                None
            }
            answers = receiver => answers.ok(),
        }
    }

    pub(crate) fn answer_question(
        &self,
        inner: &Inner,
        id: &QuestionId,
        answers: Option<Answers>,
    ) -> bool {
        let removed = self.state.lock().questions.remove(id);
        let Some(pending) = removed else {
            return false;
        };
        let answered = answers.is_some();
        if pending.reply.send(answers).is_err() {
            tracing::debug!(question = %id, "the question was abandoned before its answer");
        }
        let session = pending.ask.session;
        inner.emit(EngineEvent::QuestionResolved {
            id: id.clone(),
            session,
            answered,
        });
        true
    }

    pub(crate) fn pending_permissions(&self) -> Vec<PermissionAsk> {
        let mut asks: Vec<PermissionAsk> = self
            .state
            .lock()
            .permissions
            .values()
            .map(|pending| pending.ask.clone())
            .collect();
        asks.sort_by(|a, b| a.id.cmp(&b.id));
        asks
    }

    pub(crate) fn pending_questions(&self) -> Vec<QuestionAsk> {
        let mut asks: Vec<QuestionAsk> = self
            .state
            .lock()
            .questions
            .values()
            .map(|pending| pending.ask.clone())
            .collect();
        asks.sort_by(|a, b| a.id.cmp(&b.id));
        asks
    }

    pub(crate) fn cancel_session(&self, inner: &Inner, session: &SessionId) {
        let (permissions, questions) = {
            let mut state = self.state.lock();
            let permissions = take_where(&mut state.permissions, |pending| {
                &pending.ask.session == session
            });
            let questions = take_where(&mut state.questions, |pending| {
                &pending.ask.session == session
            });
            drop(state);
            (permissions, questions)
        };
        for pending in permissions {
            let (id, session) = (pending.ask.id, pending.ask.session);
            let reply = Reply::Reject { feedback: None };
            inner.emit(EngineEvent::PermissionResolved { id, session, reply });
        }
        for pending in questions {
            let (id, session) = (pending.ask.id, pending.ask.session);
            inner.emit(EngineEvent::QuestionResolved {
                id,
                session,
                answered: false,
            });
        }
    }

    pub(crate) fn cancel_all(&self) {
        let mut state = self.state.lock();
        state.permissions.clear();
        state.questions.clear();
    }
}

fn covered(state: &mut State, granting: &PermissionAsk) -> Vec<PendingPermission> {
    let granted: Ruleset = granting
        .always
        .iter()
        .map(|pattern| Rule::new(granting.permission.clone(), pattern.clone(), Action::Allow))
        .collect();
    take_where(&mut state.permissions, |other| {
        let patterns = other.ask.patterns.iter().map(String::as_str);
        other.ask.session == granting.session
            && granted.evaluate_all(&other.ask.permission, patterns) == Action::Allow
    })
}

fn take_where<K, V>(map: &mut HashMap<K, V>, matches: impl Fn(&V) -> bool) -> Vec<V>
where
    K: Clone + Eq + Hash,
{
    let keys: Vec<K> = map
        .iter()
        .filter(|(_, value)| matches(value))
        .map(|(key, _)| key.clone())
        .collect();
    keys.iter().filter_map(|key| map.remove(key)).collect()
}

fn deliver(sender: oneshot::Sender<Reply>, reply: Reply, id: &PermissionId) {
    if sender.send(reply).is_err() {
        tracing::debug!(permission = %id, "the permission request was abandoned before its reply");
    }
}
