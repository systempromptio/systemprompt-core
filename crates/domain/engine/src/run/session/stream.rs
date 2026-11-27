//! Consuming one step's event stream: deltas are written to the store as they
//! arrive, tool calls are collected, and a failure part-way through is retried
//! from the start when it is transient.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::llm::{Event, FinishReason, Request, Usage};
use crate::store::{MessageId, Part, PartBody, PartId, StoreError, ToolState};
use futures_util::StreamExt;
use serde_json::{Map, Value, json};
use systemprompt_identifiers::engine::CallId;

use super::model_call::{self, Attempts};
use super::run::StepContext;
use crate::run::error::EngineError;
use crate::run::governor::ModelCall;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingCall {
    pub(crate) id: CallId,
    pub(crate) name: String,
    // JSON: the tool arguments as the model wrote them; each tool checks them against its schema.
    pub(crate) input: Value,
    pub(crate) malformed: Option<String>,
    pub(crate) part_id: PartId,
}

#[derive(Debug, Default)]
pub(crate) struct Streamed {
    pub(crate) calls: Vec<PendingCall>,
    pub(crate) usage: Usage,
    pub(crate) finish: Option<FinishReason>,
}

pub(crate) async fn consume(
    ctx: &StepContext,
    call: &ModelCall,
    request: &Request,
    message_id: &MessageId,
) -> Result<Streamed, EngineError> {
    let mut attempts = Attempts::default();
    loop {
        let mut events =
            model_call::open(&ctx.inner, call, request, &ctx.cancel, &mut attempts).await?;
        let mut collector = Collector::new(ctx, message_id);
        let mut failure = None;
        while let Some(item) = events.next().await {
            match item {
                Ok(event) => collector.apply(event).await?,
                Err(error) => {
                    failure = Some(error);
                    break;
                },
            }
        }
        let Some(error) = failure else {
            return Ok(collector.finish());
        };
        if !model_call::retryable(&error, &attempts) {
            return Err(EngineError::from_llm(error));
        }
        collector.discard().await;
        model_call::backoff(&ctx.inner, call, &attempts, &error, &ctx.cancel).await?;
    }
}

struct ToolSlot {
    raw: String,
    id: CallId,
    part: PartId,
}

struct Collector<'a> {
    ctx: &'a StepContext,
    message_id: &'a MessageId,
    text: Option<PartId>,
    reasoning: Option<(Part, String)>,
    tools: Vec<ToolSlot>,
    created: Vec<PartId>,
    streamed: Streamed,
}

impl<'a> Collector<'a> {
    fn new(ctx: &'a StepContext, message_id: &'a MessageId) -> Self {
        Self {
            ctx,
            message_id,
            text: None,
            reasoning: None,
            tools: Vec::new(),
            created: Vec::new(),
            streamed: Streamed::default(),
        }
    }

    fn part(&self, body: PartBody) -> Part {
        Part::new(
            self.ctx.inner.store.ids(),
            self.ctx.session.id.clone(),
            self.message_id.clone(),
            body,
        )
    }

    async fn put(&self, part: Part) -> Result<(), EngineError> {
        self.ctx
            .inner
            .db
            .run(move |store| store.upsert_part(&part))
            .await?;
        Ok(())
    }

    async fn create(&mut self, body: PartBody) -> Result<Part, EngineError> {
        let part = self.part(body);
        self.created.push(part.id.clone());
        self.put(part.clone()).await?;
        Ok(part)
    }

    async fn append(&self, id: &PartId, delta: String) -> Result<(), EngineError> {
        let id = id.clone();
        self.ctx
            .inner
            .db
            .run(move |store| store.append_text(&id, &delta))
            .await
    }

    async fn apply(&mut self, event: Event) -> Result<(), EngineError> {
        match event {
            Event::TextDelta(delta) => self.text_delta(delta).await?,
            Event::ReasoningDelta { text, signature } => {
                self.reasoning_delta(text, signature).await?;
            },
            Event::RedactedReasoning { data } => {
                self.text = None;
                self.reasoning = None;
                let provider_data = Some(json!({ "redacted": data }));
                self.create(PartBody::Reasoning {
                    text: String::new(),
                    provider_data,
                })
                .await?;
            },
            Event::ToolCallStart { id, name } => {
                self.text = None;
                self.reasoning = None;
                self.tool_slot(&id, &name).await?;
            },
            Event::ToolCallDelta { .. } => {},
            Event::ToolCallEnd {
                id,
                name,
                input,
                malformed,
            } => {
                self.text = None;
                self.reasoning = None;
                let (call_id, part_id) = self.tool_slot(&id, &name).await?;
                let mut part = self.part(tool_body(&call_id, &name, input.clone()));
                part.id = part_id.clone();
                self.put(part).await?;
                let pending = PendingCall {
                    id: call_id,
                    name,
                    input,
                    malformed,
                    part_id,
                };
                self.streamed.calls.push(pending);
            },
            Event::Usage(usage) => self.streamed.usage = usage,
            Event::Finish { reason } => self.streamed.finish = Some(reason),
        }
        Ok(())
    }

    async fn text_delta(&mut self, delta: String) -> Result<(), EngineError> {
        if delta.is_empty() {
            return Ok(());
        }
        self.reasoning = None;
        if let Some(id) = self.text.clone() {
            self.append(&id, delta).await?;
        } else {
            let part = self
                .create(PartBody::Text {
                    text: delta,
                    synthetic: false,
                })
                .await?;
            self.text = Some(part.id);
        }
        Ok(())
    }

    async fn reasoning_delta(
        &mut self,
        text: String,
        signature: Option<String>,
    ) -> Result<(), EngineError> {
        self.text = None;
        if let Some((part, collected)) = self.reasoning.as_mut() {
            collected.push_str(&text);
            let id = part.id.clone();
            if !text.is_empty() {
                self.append(&id, text).await?;
            }
        } else {
            let body = PartBody::Reasoning {
                text: text.clone(),
                provider_data: None,
            };
            let part = self.create(body).await?;
            self.reasoning = Some((part, text));
        }
        if let Some(signature) = signature
            && let Some((mut part, collected)) = self.reasoning.take()
        {
            let provider_data = Some(json!({ "signature": signature }));
            part.body = PartBody::Reasoning {
                text: collected,
                provider_data,
            };
            self.put(part).await?;
        }
        Ok(())
    }

    async fn tool_slot(&mut self, raw: &str, name: &str) -> Result<(CallId, PartId), EngineError> {
        if let Some(slot) = self.tools.iter().find(|slot| slot.raw == raw) {
            return Ok((slot.id.clone(), slot.part.clone()));
        }
        let id = call_id(raw, self.tools.len())?;
        let part = self
            .create(tool_body(&id, name, Value::Object(Map::new())))
            .await?;
        let slot = ToolSlot {
            raw: raw.to_owned(),
            id: id.clone(),
            part: part.id.clone(),
        };
        self.tools.push(slot);
        Ok((id, part.id))
    }

    async fn discard(self) {
        for id in self.created {
            let key = id.clone();
            if let Err(error) = self
                .ctx
                .inner
                .db
                .run(move |store| store.delete_part(&key))
                .await
            {
                tracing::debug!(part = %id, error = %error, "part of a failed attempt kept");
            }
        }
    }

    fn finish(self) -> Streamed {
        self.streamed
    }
}

fn call_id(raw: &str, index: usize) -> Result<CallId, EngineError> {
    CallId::try_new(raw)
        .or_else(|_| CallId::try_new(format!("call_{index}")))
        .map_err(|error| EngineError::Store(StoreError::from(error)))
}

// JSON: the tool arguments as the model wrote them; each tool checks them
// against its schema.
fn tool_body(call_id: &CallId, name: &str, input: Value) -> PartBody {
    let state = Box::new(ToolState::Pending { input });
    PartBody::Tool {
        call_id: call_id.clone(),
        tool: name.to_owned(),
        state,
    }
}
