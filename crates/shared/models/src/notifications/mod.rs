//! Notification and subscription models.
//!
//! Producers append a [`NotificationEvent`] to the outbox for the
//! [`NOTIFICATIONS_CONSUMER`]; routing yields a [`RoutePlan`] of
//! [`RecipientDecision`]s at an [`Attention`] level, and each recipient
//! receives one [`Notification`]. Streams, subscriptions and rules are
//! declared in YAML ([`declarations`]) and checked by [`validate`].
//! [`AttentionReason`], [`AudienceSubject`], [`NotificationKind`] and
//! [`KeysetCursor`] travel as single strings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod action;
pub mod attention;
pub mod cursor;
pub mod declarations;
pub mod event;
pub mod kind;
pub mod notification;
pub mod realtime;
pub mod receipt;
pub mod route;
pub mod simulate;
mod string_form;
pub mod subject;

pub use action::ActionPrompt;
pub use attention::{Attention, AttentionReason, UnknownNotificationValue};
pub use cursor::{KeysetCursor, KeysetCursorError};
pub use declarations::{
    DeclError, DigestWindow, NotificationsDoc, QuietHours, RuleAction, RuleDecl, RuleMatch,
    StreamAttention, StreamDecl, SubscriptionDecl, SubscriptionTarget, SubscriptionTargetError,
    validate,
};
pub use event::{
    DirectRecipient, NOTIFICATION_EVENT_VERSION, NOTIFICATIONS_CONSUMER, NotificationEvent,
};
pub use kind::{NotificationKind, NotificationKindError};
pub use notification::{
    DeliveryState, Escalate, MarkRead, MarkSeen, Notification, NotificationPage,
};
pub use realtime::{
    NOTIFICATION_CREATED, NOTIFICATION_ESCALATED, NOTIFICATION_READ, NotificationCreatedPayload,
    NotificationEscalatedPayload, NotificationReadPayload,
};
pub use receipt::{
    Receipt, ReceiptState, SURFACE_AGUI, SURFACE_API, SURFACE_BRIDGE, SURFACE_DASHBOARD,
    SURFACE_MCP,
};
pub use route::{RecipientDecision, RoutePlan};
pub use simulate::{SimulateRequest, SimulateResponse, SimulatedRecipient};
pub use subject::{AudienceSubject, SubjectRef};
