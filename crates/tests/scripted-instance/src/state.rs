//! What a started instance holds and its router reads: the fixtures, the
//! keys, the request log and the script cursor.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use axum::http::{HeaderMap, Method};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::watch;

use crate::guard;
use crate::manifest::ManifestFixture;
use crate::model::ModelFixture;
use crate::record::{RecordedBody, RequestRecord, Route};
use crate::script::{ModelScript, Turn};
use crate::token::Keys;
use crate::user::{TestPat, TestUser};
use crate::verdict::VerdictScript;

#[derive(Debug)]
pub(crate) struct InstanceState {
    pub(crate) user: TestUser,
    pub(crate) pat: TestPat,
    pub(crate) manifest: ManifestFixture,
    pub(crate) models: Vec<ModelFixture>,
    pub(crate) verdict: VerdictScript,
    pub(crate) script: ModelScript,
    pub(crate) base_url: String,
    pub(crate) keys: Keys,
    pub(crate) hook_secret: String,
    pub(crate) requests: Mutex<Vec<RequestRecord>>,
    pub(crate) consumed_turns: AtomicUsize,
    pub(crate) arrivals: watch::Sender<u64>,
    pub(crate) codes: Mutex<Vec<String>>,
    pub(crate) manifests_served: AtomicUsize,
    pub(crate) manifest_dated: Mutex<DateTime<Utc>>,
}

#[derive(Debug)]
pub(crate) struct Arrival {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: RecordedBody,
}

#[derive(Debug, Clone)]
pub(crate) enum Decision {
    Turn(Box<Turn>),
    Refused(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Admitted {
    pub(crate) sequence: u64,
    pub(crate) body: RecordedBody,
    pub(crate) decision: Option<Decision>,
}

impl Admitted {
    pub(crate) fn json(&self) -> &Value {
        self.body.json().unwrap_or(&Value::Null)
    }
}

impl InstanceState {
    pub(crate) fn admit(&self, arrival: Arrival, wants_turn: bool) -> Admitted {
        let route = Route::of(&arrival.path);
        let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
        let sequence = u64::try_from(requests.len()).unwrap_or(u64::MAX);
        let (decision, purpose) = if wants_turn {
            let (decision, purpose) = self.decide(arrival.body.json().unwrap_or(&Value::Null));
            (Some(decision), purpose)
        } else {
            (None, None)
        };
        let admitted = Admitted {
            sequence,
            body: arrival.body.clone(),
            decision,
        };
        requests.push(RequestRecord {
            sequence,
            route,
            method: arrival.method,
            path: arrival.path,
            query: arrival.query,
            headers: arrival.headers,
            body: arrival.body,
            unscripted: matches!(admitted.decision, Some(Decision::Refused(_))),
            purpose,
        });
        drop(requests);
        self.arrivals.send_modify(|count| *count += 1);
        admitted
    }

    fn decide(&self, request: &Value) -> (Decision, Option<String>) {
        match self.script.aside_for(&guard::system_text(request)) {
            Some(aside) => {
                let answer = Turn {
                    respond: aside.respond.clone(),
                    ..Turn::default()
                };
                (
                    Decision::Turn(Box::new(answer)),
                    Some(aside.purpose.clone()),
                )
            },
            None => (self.next_turn(request), None),
        }
    }

    fn next_turn(&self, request: &Value) -> Decision {
        let index = self.consumed_turns.load(Ordering::SeqCst);
        let Some(turn) = self.script.turns.get(index) else {
            return Decision::Refused(format!(
                "the script has no turn left for this request ({index} consumed)"
            ));
        };
        match guard::check(&turn.expect, request) {
            Ok(()) => {
                self.consumed_turns.store(index + 1, Ordering::SeqCst);
                Decision::Turn(Box::new(turn.clone()))
            },
            Err(mismatch) => {
                tracing::error!(turn = index, %mismatch, "model request does not match its turn");
                Decision::Refused(format!("turn {index}: {mismatch}"))
            },
        }
    }

    pub(crate) fn issue_code(&self) -> String {
        let code = format!("code-{:032x}", rand::random::<u128>());
        self.codes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(code.clone());
        code
    }

    pub(crate) fn redeem_code(&self, code: &str) -> bool {
        let mut codes = self.codes.lock().unwrap_or_else(PoisonError::into_inner);
        let found = codes.iter().position(|issued| issued == code);
        found.map(|at| codes.swap_remove(at)).is_some()
    }
}
