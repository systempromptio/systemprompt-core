//! The parity comparison of the golden scenarios
//! (`documentation/future/harness/testing/03-golden-scenarios.md` sections 3
//! and 6).
//!
//! Each product's transcript is first read into a [`Transcript`], a form that
//! names no product: the harness's event stream and the scripted instance's
//! request log ([`harness`]), or the reference runner's export
//! ([`reference`]). [`build::trace`] reduces a transcript and the scenario's
//! script to the normalised trace, [`compare::differences`] lists where two
//! traces part, and [`ledger`] classifies every difference against the
//! reviewed file beside this crate and writes the parity ledger.

pub mod build;
pub mod compare;
pub mod digest;
pub mod harness;
pub mod ledger;
pub mod reference;
pub mod script;
pub mod summary;
pub mod transcript;

pub use compare::{Difference, differences};
pub use ledger::{Classification, Classifications, Ledger, LedgerRow};
pub use script::ScenarioScript;
pub use transcript::{
    Call, CallStatus, Mark, MarkKind, Message, ModelRequest, Reply, SessionLog, Transcript,
};
