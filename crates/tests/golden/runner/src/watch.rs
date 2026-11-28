//! The watcher of a run's event stream: it performs the scenario's actions on
//! the events they name through the run's control feed and records each with
//! the number of model requests the instance had received.

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::Value;
use systemprompt_harness::command::{ControlCommand, ControlError};
use systemprompt_parity_compare::{Mark, MarkKind};
use systemprompt_scripted_instance::ScriptedInstance;
use tokio::sync::mpsc;

use crate::scenario::{ActionOn, Scenario};

pub(crate) struct LineSink {
    pub(crate) pending: Vec<u8>,
    pub(crate) lines: mpsc::UnboundedSender<String>,
}

impl Write for LineSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line[..end]).into_owned();
            if self.lines.send(line).is_err() {
                return Err(std::io::Error::other("the watcher stopped"));
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn model_calls(instance: &ScriptedInstance) -> usize {
    instance
        .requests()
        .iter()
        .filter(|record| crate::outcome::is_step_call(record))
        .count()
}

pub(crate) struct Watcher<'a> {
    scenario: &'a Scenario,
    instance: &'a ScriptedInstance,
    controls: Option<mpsc::UnboundedSender<Result<ControlCommand, ControlError>>>,
    pub(crate) events: Vec<Value>,
    pub(crate) marks: Vec<Mark>,
    done: Vec<bool>,
    root: String,
    roles: BTreeMap<String, (String, String)>,
    user_messages: Vec<String>,
    responses: Vec<String>,
    texts: Vec<String>,
    asks: usize,
    idles: usize,
    busy: bool,
    closing_after_revert: bool,
}

impl<'a> Watcher<'a> {
    pub(crate) fn new(
        scenario: &'a Scenario,
        instance: &'a ScriptedInstance,
        controls: mpsc::UnboundedSender<Result<ControlCommand, ControlError>>,
    ) -> Self {
        Self {
            scenario,
            instance,
            controls: Some(controls),
            events: Vec::new(),
            marks: Vec::new(),
            done: vec![false; scenario.actions.len()],
            root: String::new(),
            roles: BTreeMap::new(),
            user_messages: Vec::new(),
            responses: Vec::new(),
            texts: Vec::new(),
            asks: 0,
            idles: 0,
            busy: false,
            closing_after_revert: false,
        }
    }

    pub(crate) async fn watch(&mut self, mut lines: mpsc::UnboundedReceiver<String>) {
        while let Some(line) = lines.recv().await {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let finished = self.observe(&event);
            self.events.push(event);
            if finished {
                break;
            }
        }
    }

    fn field<'v>(event: &'v Value, pointer: &str) -> &'v str {
        event
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    fn observe(&mut self, event: &Value) -> bool {
        match Self::field(event, "/type") {
            "started" => self.root = Self::field(event, "/session_id").to_owned(),
            "session"
                if self.closing_after_revert
                    && Self::field(event, "/session/id") == self.root
                    && event
                        .pointer("/session/revert")
                        .is_some_and(|revert| !revert.is_null()) =>
            {
                self.closing_after_revert = false;
                self.close();
            },
            "message" => {
                let id = Self::field(event, "/message/id").to_owned();
                let session = Self::field(event, "/message/session_id").to_owned();
                let role = Self::field(event, "/message/role").to_owned();
                let system =
                    event.pointer("/message/system").and_then(Value::as_bool) == Some(true);
                if role == "user"
                    && session == self.root
                    && !system
                    && !self.user_messages.contains(&id)
                {
                    self.user_messages.push(id.clone());
                }
                if role == "assistant" && session == self.root && !self.responses.contains(&id) {
                    self.responses.push(id.clone());
                }
                self.roles.insert(id, (session, role));
            },
            "part" if Self::field(event, "/part/body/type") == "text" => {
                let message = Self::field(event, "/part/message_id").to_owned();
                let ours = self
                    .roles
                    .get(&message)
                    .is_some_and(|(session, role)| *session == self.root && role == "assistant");
                if ours
                    && !Self::field(event, "/part/body/text").is_empty()
                    && !self.texts.contains(&message)
                {
                    let response = self
                        .responses
                        .iter()
                        .position(|id| *id == message)
                        .map_or(0, |at| at + 1);
                    self.texts.push(message);
                    self.fire(ActionOn::Text, response);
                }
            },
            "permission_asked" => {
                self.asks += 1;
                self.fire(ActionOn::Ask, self.asks);
            },
            "status" if Self::field(event, "/session_id") == self.root => {
                match Self::field(event, "/status/type") {
                    "idle" if self.busy => {
                        self.busy = false;
                        self.idles += 1;
                        self.fire(ActionOn::Idle, self.idles);
                        let more = self
                            .scenario
                            .actions
                            .iter()
                            .zip(&self.done)
                            .any(|(action, done)| !done && action.on == ActionOn::Idle);
                        let reverting = self
                            .marks
                            .last()
                            .is_some_and(|mark| mark.kind == MarkKind::Revert);
                        if !more && reverting {
                            self.closing_after_revert = true;
                        } else if !more {
                            self.close();
                        }
                    },
                    "idle" => {},
                    _ => self.busy = true,
                }
            },
            "finished" => return true,
            _ => {},
        }
        false
    }

    fn close(&mut self) {
        self.send(ControlCommand::End {});
        self.controls = None;
    }

    fn send(&self, control: ControlCommand) {
        if let Some(controls) = &self.controls
            && controls.send(Ok(control)).is_err()
        {
            eprintln!("the run no longer takes controls");
        }
    }

    fn fire(&mut self, on: ActionOn, index: usize) {
        for at in 0..self.scenario.actions.len() {
            let action = &self.scenario.actions[at];
            if self.done[at] || action.on != on || action.index != index {
                continue;
            }
            self.done[at] = true;
            let message = action.message;
            let text = message
                .and_then(|number| self.scenario.messages.get(number as usize - 1))
                .cloned()
                .unwrap_or_default();
            let (kind, control) = match action.act.as_str() {
                "steer" => (MarkKind::Steer, ControlCommand::Steer { text }),
                "prompt" => (MarkKind::Prompt, ControlCommand::Prompt { text }),
                "abort" => (MarkKind::Abort, ControlCommand::Abort {}),
                _ => {
                    let Some(number) = message else {
                        eprintln!("a revert action names no message");
                        continue;
                    };
                    (
                        MarkKind::Revert,
                        ControlCommand::Revert {
                            prompt: number as usize,
                        },
                    )
                },
            };
            self.marks.push(Mark {
                kind,
                requests_before: model_calls(self.instance),
                message,
            });
            self.send(control);
        }
    }
}
