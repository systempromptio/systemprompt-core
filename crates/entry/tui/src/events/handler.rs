use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::messages::Message;

pub struct EventHandler {
    tx: mpsc::UnboundedSender<Message>,
    tick_rate: Duration,
}

impl std::fmt::Debug for EventHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventHandler")
            .field("tick_rate", &self.tick_rate)
            .finish_non_exhaustive()
    }
}

impl EventHandler {
    pub const fn new(tx: mpsc::UnboundedSender<Message>, tick_rate: Duration) -> Self {
        Self { tx, tick_rate }
    }

    pub async fn run(&self) -> Result<()> {
        let mut event_stream = EventStream::new();
        let mut tick_interval = interval(self.tick_rate);

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if self.tx.send(Message::Tick).is_err() {
                        break;
                    }
                }

                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            let message = match event {
                                Event::Key(key) => Message::Key(key),
                                Event::Mouse(mouse) => Message::Mouse(mouse),
                                Event::Resize(w, h) => Message::Resize(w, h),
                                _ => continue,
                            };

                            if self.tx.send(message).is_err() {
                                break;
                            }
                        }
                        Some(Err(_)) | None => {
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
