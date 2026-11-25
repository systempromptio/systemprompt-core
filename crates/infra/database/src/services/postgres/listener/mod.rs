//! [`NotificationHub`]: the node's one Postgres `LISTEN` session.
//!
//! Every subscriber on a node — the cross-replica event relay, the
//! process-ownership wake, the durable outbox consumers' wakes and the
//! agent-run event fan-out — reads its channels from one listener the hub
//! holds on the session pool, so a node spends one connection on `LISTEN`
//! however many subscribers it has. A hub the node has bound
//! ([`NotificationHub::bind`], the composition root's [`BackgroundTasks`]
//! and session pool) runs the listener as one owned task from its first
//! subscription; an unbound hub (a CLI one-shot, a hosted child) never opens
//! a session, and its subscriptions see the link as down and poll.
//!
//! [`PollWake`] is the consumer side most subscribers need: wait for a
//! notification on its channels or the poll interval, whichever comes first,
//! so polling is the delivery guarantee and the notification only makes it
//! prompt.
//!
//! A notification is only a wake-up. The listener never reconnects inside a
//! receive: a lost session takes the link down, every subscription sees
//! [`HubEvent::Lost`], and each falls back to polling (or, for the relay,
//! replays) until a new session is listening again, reported as
//! [`HubEvent::Listening`] once every channel the subscription named is in
//! place. A subscriber that falls behind the fan-out buffer sees
//! [`HubEvent::Lost`] too, because it may have missed a notification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_traits::BackgroundTasks;
use tokio::sync::{Notify, broadcast, watch};
use tokio_util::sync::CancellationToken;

mod drain_next;
mod session;

pub use drain_next::DrainNext;

const FAN_OUT_CAPACITY: usize = 4096;

/// The state of the hub's `LISTEN` session as subscriptions see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenLink {
    Down,
    Up {
        session: u64,
        channels: Arc<BTreeSet<&'static str>>,
    },
}

/// One notification the hub received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubNotification {
    pub channel: Arc<str>,
    pub payload: Arc<str>,
}

/// What a [`HubSubscription::recv`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubEvent {
    Notification(Arc<HubNotification>),
    Listening { session: u64 },
    Lost,
}

#[derive(Debug)]
struct HubState {
    channels: Mutex<BTreeSet<&'static str>>,
    channels_changed: Notify,
    fan_out: broadcast::Sender<Arc<HubNotification>>,
    link: watch::Sender<ListenLink>,
    runner: Mutex<Option<(BackgroundTasks, PgPool)>>,
    started: AtomicBool,
}

/// The node's one `LISTEN` session, shared by every subscriber.
#[derive(Debug, Clone)]
pub struct NotificationHub {
    state: Arc<HubState>,
}

impl Default for NotificationHub {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationHub {
    #[must_use]
    pub fn new() -> Self {
        let (fan_out, _) = broadcast::channel(FAN_OUT_CAPACITY);
        Self {
            state: Arc::new(HubState {
                channels: Mutex::new(BTreeSet::new()),
                channels_changed: Notify::new(),
                fan_out,
                link: watch::Sender::new(ListenLink::Down),
                runner: Mutex::new(None),
                started: AtomicBool::new(false),
            }),
        }
    }

    #[must_use]
    pub fn subscribe(&self, channels: &[&'static str]) -> HubSubscription {
        let wanted: BTreeSet<&'static str> = channels.iter().copied().collect();
        let added = {
            let mut registered = self
                .state
                .channels
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let before = registered.len();
            registered.extend(wanted.iter().copied());
            registered.len() != before
        };
        if added {
            self.state.channels_changed.notify_one();
        }
        self.start_if_wanted();
        HubSubscription {
            channels: wanted,
            notifications: self.state.fan_out.subscribe(),
            link: self.state.link.subscribe(),
            reported: None,
        }
    }

    #[must_use]
    pub fn link(&self) -> ListenLink {
        self.state.link.borrow().clone()
    }

    #[must_use]
    pub fn is_started(&self) -> bool {
        self.state.started.load(Ordering::Acquire)
    }

    pub fn bind(&self, tasks: &BackgroundTasks, pool: PgPool) {
        *self
            .state
            .runner
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((tasks.clone(), pool));
        self.start_if_wanted();
    }

    fn start_if_wanted(&self) {
        let wanted = !self
            .state
            .channels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty();
        if !wanted {
            return;
        }
        let Some((tasks, pool)) = self
            .state
            .runner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        else {
            return;
        };
        if self.state.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let state = Arc::clone(&self.state);
        tasks.spawn_cancellable("notification_hub", move |cancel| {
            session::run(state, pool, cancel)
        });
    }
}

/// One subscriber's view of the hub: its channels' notifications and the
/// link's transitions.
#[derive(Debug)]
pub struct HubSubscription {
    channels: BTreeSet<&'static str>,
    notifications: broadcast::Receiver<Arc<HubNotification>>,
    link: watch::Receiver<ListenLink>,
    reported: Option<u64>,
}

impl HubSubscription {
    #[must_use]
    pub fn is_listening(&self) -> bool {
        self.listening_session(&self.link.borrow()).is_some()
    }

    fn listening_session(&self, link: &ListenLink) -> Option<u64> {
        match link {
            ListenLink::Up { session, channels } if self.channels.is_subset(channels) => {
                Some(*session)
            },
            _ => None,
        }
    }

    pub async fn recv(&mut self) -> HubEvent {
        loop {
            let link = self.link.borrow_and_update().clone();
            let current = self.listening_session(&link);
            match (self.reported, current) {
                (Some(_), None) => {
                    self.reported = None;
                    return HubEvent::Lost;
                },
                (reported, Some(session)) if reported != Some(session) => {
                    self.reported = Some(session);
                    self.notifications = self.notifications.resubscribe();
                    return HubEvent::Listening { session };
                },
                _ => {},
            }
            tokio::select! {
                changed = self.link.changed() => {
                    if changed.is_err() {
                        std::future::pending::<()>().await;
                    }
                },
                received = self.notifications.recv(), if self.reported.is_some() => match received {
                    Ok(notification) if self.channels.contains(notification.channel.as_ref()) => {
                        return HubEvent::Notification(notification);
                    },
                    Ok(_) => {},
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        self.reported = None;
                        return HubEvent::Lost;
                    },
                    Err(broadcast::error::RecvError::Closed) => {
                        std::future::pending::<()>().await;
                    },
                },
            }
        }
    }
}

/// Why a [`PollWake::wait`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    Notified,
    Polled,
    Shutdown,
}

/// A subscriber's wait: the first of a notification on its channels, a
/// (re)connected session, the poll interval or shutdown.
#[derive(Debug)]
pub struct PollWake {
    subscription: HubSubscription,
}

impl PollWake {
    #[must_use]
    pub fn new(hub: &NotificationHub, channels: &[&'static str]) -> Self {
        Self {
            subscription: hub.subscribe(channels),
        }
    }

    #[must_use]
    pub fn is_listening(&self) -> bool {
        self.subscription.is_listening()
    }

    pub async fn wait(&mut self, poll: Duration, shutdown: &CancellationToken) -> WakeReason {
        let deadline = tokio::time::sleep(poll);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return WakeReason::Shutdown,
                () = &mut deadline => return WakeReason::Polled,
                event = self.subscription.recv() => match event {
                    HubEvent::Notification(_) | HubEvent::Listening { .. } => return WakeReason::Notified,
                    HubEvent::Lost => {},
                },
            }
        }
    }
}
