//! Writing a conversation straight into the store, so a test can put a session
//! in a state no scripted run leaves it in (a reply that failed after a tool
//! ran, reasoning with replay data, a call that never finished) and then read
//! what the next request makes of it.

use serde_json::json;
use systemprompt_engine::run::Engine;
use systemprompt_engine::store::{
    AssistantMessage, CallId, FinishReason, Message, MessageId, Part, PartBody, SessionId, Store,
    ToolState, UserMessage, now_ms,
};

use crate::support::MODEL;

const AGENT: &str = "build";

pub(super) struct Seed<'a> {
    store: &'a Store,
    session: SessionId,
    parent: Option<MessageId>,
}

impl<'a> Seed<'a> {
    pub(super) fn new(engine: &'a Engine, session: &SessionId) -> Self {
        Self {
            store: engine.store(),
            session: session.clone(),
            parent: None,
        }
    }

    fn put(&self, message: &Message, bodies: Vec<PartBody>) {
        self.store.upsert_message(message).unwrap();
        for body in bodies {
            let part = Part::new(
                self.store.ids(),
                self.session.clone(),
                message.id().clone(),
                body,
            );
            self.store.upsert_part(&part).unwrap();
        }
    }

    fn write_user(&mut self, system: bool, bodies: Vec<PartBody>) {
        let mut message = UserMessage::new(self.store.ids(), self.session.clone(), AGENT, MODEL);
        message.system = system;
        self.parent = Some(message.id.clone());
        self.put(&Message::User(message), bodies);
    }

    /// A prompt the user sent.
    pub(super) fn user(&mut self, bodies: Vec<PartBody>) {
        self.write_user(false, bodies);
    }

    /// A user-role message the engine wrote itself.
    pub(super) fn engine_user(&mut self, bodies: Vec<PartBody>) {
        self.write_user(true, bodies);
    }

    /// A reply to the latest user message that finished normally.
    pub(super) fn reply(&self, bodies: Vec<PartBody>) {
        self.reply_with(|_| {}, bodies);
    }

    /// A reply to the latest user message, finished normally unless `edit`
    /// says otherwise.
    pub(super) fn reply_with(
        &self,
        edit: impl FnOnce(&mut AssistantMessage),
        bodies: Vec<PartBody>,
    ) {
        let parent = self.parent.clone().expect("a user message comes first");
        let mut message =
            AssistantMessage::new(self.store.ids(), self.session.clone(), parent, AGENT, MODEL);
        message.finish = Some(FinishReason::Stop);
        message.completed_at = Some(now_ms());
        edit(&mut message);
        self.put(&Message::Assistant(message), bodies);
    }
}

pub(super) fn text(text: &str) -> PartBody {
    PartBody::Text {
        text: text.to_owned(),
        synthetic: false,
    }
}

pub(super) fn reasoning(text: &str, signature: Option<&str>) -> PartBody {
    PartBody::Reasoning {
        text: text.to_owned(),
        provider_data: signature.map(|signature| json!({ "signature": signature })),
    }
}

pub(super) fn tool(call: &str, name: &str, state: ToolState) -> PartBody {
    PartBody::Tool {
        call_id: CallId::try_new(call).unwrap(),
        tool: name.to_owned(),
        state: Box::new(state),
    }
}
