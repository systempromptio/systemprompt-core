//! What a started instance holds and its router reads.

use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;

use crate::manifest::ManifestFixture;
use crate::model::ModelFixture;
use crate::record::RequestRecord;
use crate::script::ModelScript;
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
    pub(crate) requests: Mutex<Vec<RequestRecord>>,
    pub(crate) consumed_turns: AtomicUsize,
}
