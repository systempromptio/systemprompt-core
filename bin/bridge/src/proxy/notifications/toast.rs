//! Which inbox items become desktop toasts, decided without any windowing code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{Attention, Notification};

pub const TOAST_FLOOR: Attention = Attention::Human;
pub const CATCH_UP_TOASTS: usize = 3;

/// One toast the GUI should raise: a single item that needs the reader, or
/// the fold of everything past the per-catch-up cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxToast {
    Item(Box<ItemToast>),
    More { count: usize },
}

/// What a single-item toast shows and where its click leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemToast {
    pub id: NotificationId,
    pub actor: String,
    pub excerpt: Option<String>,
    pub open_url: Option<String>,
}

impl InboxToast {
    #[must_use]
    pub fn for_item(item: &Notification) -> Self {
        Self::Item(Box::new(ItemToast {
            id: item.id.clone(),
            actor: item
                .actor_display
                .clone()
                .unwrap_or_else(|| item.actor.to_string()),
            excerpt: item.excerpt.clone(),
            open_url: item.action.open_url.clone(),
        }))
    }
}

fn needs_reader(item: &Notification) -> bool {
    item.attention >= TOAST_FLOOR && item.read_at.is_none()
}

// Why: with no watermark this device has never toasted, so a first catch-up
// adopts the newest item as the watermark instead of replaying history.
#[must_use]
pub fn plan_toasts<'a>(
    candidates: impl IntoIterator<Item = &'a Notification>,
    through: Option<DateTime<Utc>>,
    cap: usize,
) -> (Vec<InboxToast>, Option<DateTime<Utc>>) {
    let candidates: Vec<&Notification> = candidates.into_iter().collect();
    let newest = candidates.iter().map(|item| item.created_at).max();
    let Some(through) = through else {
        return (Vec::new(), newest);
    };
    let mut due: Vec<&Notification> = candidates
        .into_iter()
        .filter(|item| item.created_at > through && needs_reader(item))
        .collect();
    due.sort_by_key(|item| item.created_at);
    let watermark = Some(newest.map_or(through, |n| n.max(through)));
    let folded = due.len().saturating_sub(cap);
    let mut toasts: Vec<InboxToast> = due
        .iter()
        .rev()
        .take(cap)
        .map(|item| InboxToast::for_item(item))
        .collect();
    if folded > 0 {
        toasts.push(InboxToast::More { count: folded });
    }
    (toasts, watermark)
}
