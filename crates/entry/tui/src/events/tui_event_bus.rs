use systemprompt_traits::{
    AnalyticsEvent as TraitAnalyticsEvent, AnalyticsEventPublisher,
    LogEventData as TraitLogEventData, LogEventLevel, LogEventPublisher,
    UserEvent as TraitUserEvent, UserEventPublisher,
};
use tokio::sync::broadcast;

use super::tui_events::{LogEventData, TuiEvent};
use crate::messages::LogLevel;

const DEFAULT_CAPACITY: usize = 256;

#[derive(Debug)]
pub struct TuiEventBus {
    sender: broadcast::Sender<TuiEvent>,
}

impl TuiEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: TuiEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TuiEvent> {
        self.sender.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<TuiEvent> {
        self.sender.clone()
    }
}

impl Default for TuiEventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl Clone for TuiEventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl LogEventPublisher for TuiEventBus {
    fn publish_log(&self, event: TraitLogEventData) {
        let level = match event.level {
            LogEventLevel::Error => LogLevel::Error,
            LogEventLevel::Warn => LogLevel::Warn,
            LogEventLevel::Info => LogLevel::Info,
            LogEventLevel::Debug => LogLevel::Debug,
            LogEventLevel::Trace => LogLevel::Trace,
        };

        let tui_event = TuiEvent::LogCreated(LogEventData::new(
            event.timestamp,
            level,
            event.module,
            event.message,
        ));

        self.publish(tui_event);
    }
}

impl UserEventPublisher for TuiEventBus {
    fn publish_user_event(&self, event: TraitUserEvent) {
        use systemprompt_identifiers::{SessionId, UserId};
        let tui_event = match event {
            TraitUserEvent::UserCreated { user_id } | TraitUserEvent::UserUpdated { user_id } => {
                TuiEvent::UserChanged {
                    user_id: UserId::new(&user_id),
                }
            },
            TraitUserEvent::SessionCreated {
                user_id,
                session_id,
            }
            | TraitUserEvent::SessionEnded {
                user_id,
                session_id,
            } => TuiEvent::SessionChanged {
                user_id: UserId::new(&user_id),
                session_id: SessionId::new(&session_id),
            },
        };

        self.publish(tui_event);
    }
}

impl AnalyticsEventPublisher for TuiEventBus {
    fn publish_analytics_event(&self, _event: TraitAnalyticsEvent) {
        self.publish(TuiEvent::AnalyticsUpdated);
    }
}
