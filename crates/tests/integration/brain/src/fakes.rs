use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use systemprompt_extension::brain::{
    BrainExtractError, BrainModelClient, BrainModelError, EventFilter, FactExtractor,
    FilterVerdict, SourceAccess, SourceAccessError, SourceContent, SourceContentError,
    SourceVerdict,
};
use systemprompt_identifiers::{AiRequestId, FactKindId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    BrainEnvelope, BrainModelRequest, BrainModelResponse, ExtractionInput, ExtractionOutput,
    ExtractionRecord, ExtractorDecl, FactCandidate, FactSource, KindDecl, SourceText, SubjectKey,
};
use systemprompt_models::notifications::SubjectRef;

pub(crate) const PARSER: &str = "test.parser";
pub(crate) const MODEL_TOKENS_IN: u32 = 40;
pub(crate) const MODEL_TOKENS_OUT: u32 = 10;

#[derive(Default)]
pub(crate) struct FakeModelClient {
    script: Mutex<VecDeque<serde_json::Value>>,
    calls: AtomicUsize,
    ai_request_id: Mutex<Option<AiRequestId>>,
}

impl FakeModelClient {
    pub(crate) fn push(&self, facts: serde_json::Value) {
        self.script.lock().expect("lock").push_back(facts);
    }

    pub(crate) fn with_request_id(&self, id: AiRequestId) {
        *self.ai_request_id.lock().expect("lock") = Some(id);
    }

    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl BrainModelClient for FakeModelClient {
    async fn complete(
        &self,
        _request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let facts = self
            .script
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_else(|| serde_json::json!([]));
        Ok(BrainModelResponse {
            model: "fake-model".to_owned(),
            text: String::new(),
            structured: Some(serde_json::json!({ "facts": facts })),
            input_tokens: MODEL_TOKENS_IN,
            output_tokens: MODEL_TOKENS_OUT,
            ai_request_id: self.ai_request_id.lock().expect("lock").clone(),
        })
    }
}

pub(crate) struct GatherFilter {
    pub(crate) kinds: Vec<FactKindId>,
}

impl EventFilter for GatherFilter {
    fn name(&self) -> &'static str {
        "test.gather"
    }

    fn decide(&self, _envelope: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Gather {
            kinds: self.kinds.clone(),
        }
    }
}

#[derive(Default)]
pub(crate) struct TestParser {
    team: Mutex<Option<(String, String)>>,
}

impl TestParser {
    pub(crate) fn emit(&self, product: &str, team: &str) {
        *self.team.lock().expect("lock") = Some((product.to_owned(), team.to_owned()));
    }
}

#[async_trait]
impl FactExtractor for TestParser {
    fn name(&self) -> &'static str {
        PARSER
    }

    fn handles(&self, kind: &KindDecl) -> bool {
        matches!(&kind.extractor, Some(ExtractorDecl::Parser { name }) if name == PARSER)
    }

    async fn extract(
        &self,
        input: &ExtractionInput,
    ) -> Result<ExtractionOutput, BrainExtractError> {
        let candidates = self
            .team
            .lock()
            .expect("lock")
            .clone()
            .map(|(product, team)| {
                FactCandidate::builder(
                    input.kind.kind.clone(),
                    SubjectKey::new(input.kind.subject.clone(), product),
                    input.source.clone(),
                )
                .field("team", serde_json::json!(team))
                .extractor(PARSER)
                .build()
            })
            .into_iter()
            .collect();
        Ok(ExtractionOutput {
            candidates,
            record: ExtractionRecord {
                extractor: PARSER.to_owned(),
                ..ExtractionRecord::default()
            },
        })
    }
}

#[derive(Default)]
pub(crate) struct FakeSourceContent {
    text: Mutex<SourceText>,
}

impl FakeSourceContent {
    pub(crate) fn set(&self, text: SourceText) {
        *self.text.lock().expect("lock") = text;
    }
}

#[async_trait]
impl SourceContent for FakeSourceContent {
    fn name(&self) -> &'static str {
        "test.source"
    }

    fn handles(&self, _source: &FactSource) -> bool {
        true
    }

    async fn fetch(&self, _source: &FactSource) -> Result<Option<SourceText>, SourceContentError> {
        Ok(Some(self.text.lock().expect("lock").clone()))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Access {
    Verdict(SourceVerdict),
    Error,
}

#[derive(Default)]
pub(crate) struct FakeSourceAccess {
    verdicts: Mutex<HashMap<(String, UserId), Access>>,
    calls: AtomicUsize,
}

impl FakeSourceAccess {
    pub(crate) fn set(&self, document: &str, reader: &UserId, access: Access) {
        self.verdicts
            .lock()
            .expect("lock")
            .insert((document.to_owned(), reader.clone()), access);
    }

    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SourceAccess for FakeSourceAccess {
    async fn readable(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
        _actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let key = (subject.id.clone(), reader.clone());
        match self.verdicts.lock().expect("lock").get(&key).copied() {
            None => Ok(SourceVerdict::Readable),
            Some(Access::Verdict(verdict)) => Ok(verdict),
            Some(Access::Error) => Err(SourceAccessError::Other {
                kind: subject.kind.clone(),
                source: "the access check failed".into(),
            }),
        }
    }
}
