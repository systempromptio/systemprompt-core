use std::convert::Infallible;
use std::future::{Ready, ready};

use systemprompt_bridge::proxy::notifications::{CATCH_UP_PAGE, InboxHandle, MAX_ITEMS, catch_up};
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{KeysetCursor, Notification, NotificationPage};

const CURSORS: [&str; 8] = [
    "MjAyNi0wOS0wMVQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwMA",
    "MjAyNi0wOS0wMlQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwMQ",
    "MjAyNi0wOS0wM1QwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwMg",
    "MjAyNi0wOS0wNFQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwMw",
    "MjAyNi0wOS0wNVQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwNA",
    "MjAyNi0wOS0wNlQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwNQ",
    "MjAyNi0wOS0wN1QwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwNg",
    "MjAyNi0wOS0wOFQwMDowMDowMC4wMDAwMDBafDAwMDAwMDAwLTAwMDAtNDAwMC05MDAwLTAwMDAwMDAwMDAwNw",
];

fn cursor(page: usize) -> KeysetCursor {
    CURSORS[page].parse().expect("cursor decodes")
}

fn nid(n: u32) -> NotificationId {
    NotificationId::new(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn item(n: u32) -> Notification {
    let at = format!(
        "2026-10-{:02}T{:02}:{:02}:00Z",
        1 + n / 1440,
        (n / 60) % 24,
        n % 60
    );
    serde_json::from_value(serde_json::json!({
        "id": nid(n).as_str(),
        "recipient": "8b1d6f0e-2c4a-4f7e-9a51-3d2e6c7b9f10",
        "kind": "document.comment.mentioned",
        "stream": null,
        "actor": "2f6e8d4a-91b3-4c5e-8f07-6a1d3b9e2c45",
        "actor_display": "Priya",
        "actor_is_agent": false,
        "via_agent": null,
        "subjects": [],
        "attention": "silent",
        "attention_reason": "sender",
        "action": {
            "prompt": "p",
            "open_url": null,
            "kind": "document.comment.mentioned",
            "actor": "2f6e8d4a-91b3-4c5e-8f07-6a1d3b9e2c45"
        },
        "excerpt": null,
        "coalesce_count": 1,
        "first_event_at": at,
        "last_event_at": at,
        "created_at": at,
        "read_at": null,
        "escalated_from": null,
        "escalation_note": null,
        "held_until": null,
        "delivery_state": "delivered",
    }))
    .expect("notification fixture decodes")
}

struct Feed {
    newest_first: Vec<Notification>,
    calls: Vec<Option<KeysetCursor>>,
}

impl Feed {
    fn new(oldest: u32, newest: u32) -> Self {
        Self {
            newest_first: (oldest..=newest).rev().map(item).collect(),
            calls: Vec::new(),
        }
    }

    fn fetch(
        &mut self,
        since: Option<KeysetCursor>,
        limit: u32,
    ) -> Ready<Result<NotificationPage, Infallible>> {
        let page = since.map_or(0, |c| {
            CURSORS
                .iter()
                .position(|raw| cursor_eq(raw, &c))
                .map_or(usize::MAX, |k| k + 1)
        });
        self.calls.push(since);
        let limit = limit as usize;
        let start = page.saturating_mul(limit).min(self.newest_first.len());
        let end = (start + limit).min(self.newest_first.len());
        let more = end < self.newest_first.len();
        ready(Ok(NotificationPage {
            notifications: self.newest_first[start..end].to_vec(),
            next_cursor: more.then(|| cursor(page)),
            unread: 0,
            unread_capped: false,
        }))
    }
}

fn cursor_eq(raw: &str, cursor: &KeysetCursor) -> bool {
    cursor.to_string() == raw
}

fn seeded(oldest: u32, newest: u32) -> std::sync::Arc<InboxHandle> {
    let (handle, _rx) = InboxHandle::new(None);
    handle.update(|state| {
        for n in oldest..=newest {
            state.upsert(item(n));
        }
    });
    handle
}

#[tokio::test]
async fn catch_up_fetches_newer_items_even_with_a_stored_older_cursor() {
    let handle = seeded(1, 3);
    handle.update(|state| state.older_cursor = Some(cursor(7)));
    let mut feed = Feed::new(1, 5);

    let fresh = catch_up(&handle, |c, l| feed.fetch(c, l))
        .await
        .expect("infallible");

    assert_eq!(feed.calls, vec![None], "catch-up starts at the newest item");
    assert_eq!(fresh, vec![nid(5), nid(4)]);
    let state = handle.snapshot();
    assert!(state.find(&nid(5)).is_some() && state.find(&nid(4)).is_some());
    assert_eq!(
        state.older_cursor,
        Some(cursor(7)),
        "load-older position kept"
    );
}

#[tokio::test]
async fn catch_up_stops_at_the_first_page_holding_a_known_id() {
    let page = CATCH_UP_PAGE;
    let handle = seeded(1, 10);
    let mut feed = Feed::new(1, 10 + page + 5);

    let fresh = catch_up(&handle, |c, l| feed.fetch(c, l))
        .await
        .expect("infallible");

    assert_eq!(feed.calls.len(), 2, "the second page overlaps the inbox");
    assert_eq!(fresh.len(), (page + 5) as usize);
    assert!(
        fresh.iter().all(|id| *id != nid(10)),
        "known ids are not fresh"
    );
}

#[tokio::test]
async fn catch_up_into_an_empty_inbox_stops_at_the_kept_cap() {
    let (handle, _rx) = InboxHandle::new(None);
    let total = u32::try_from(MAX_ITEMS).expect("cap fits") * 2;
    let mut feed = Feed::new(1, total);

    catch_up(&handle, |c, l| feed.fetch(c, l))
        .await
        .expect("infallible");

    assert_eq!(feed.calls.len(), MAX_ITEMS / CATCH_UP_PAGE as usize);
    let state = handle.snapshot();
    assert_eq!(state.items.len(), MAX_ITEMS);
    assert_eq!(state.items[0].id, nid(total), "newest item kept first");
    assert!(
        state.older_cursor.is_some(),
        "the cut leaves a load-older cursor"
    );
}

#[tokio::test]
async fn catch_up_never_claims_seen_receipts() {
    let (handle, _rx) = InboxHandle::new(None);
    let mut feed = Feed::new(1, 3);

    catch_up(&handle, |c, l| feed.fetch(c, l))
        .await
        .expect("infallible");

    let shown = [nid(1), nid(2), nid(3)];
    assert_eq!(handle.claim_unseen(&shown).len(), 3);
}

#[test]
fn seen_is_claimed_once_per_shown_id() {
    let handle = seeded(1, 3);
    let shown = [nid(1), nid(2), nid(99)];

    assert_eq!(handle.claim_unseen(&shown), vec![nid(1), nid(2)]);
    assert!(handle.claim_unseen(&shown).is_empty(), "already reported");
    assert_eq!(handle.claim_unseen(&[nid(2), nid(3)]), vec![nid(3)]);
}

#[test]
fn a_failed_seen_report_is_claimable_again() {
    let handle = seeded(1, 2);
    let claimed = handle.claim_unseen(&[nid(1), nid(2)]);

    handle.release_seen(&claimed);

    assert_eq!(handle.claim_unseen(&[nid(1)]), vec![nid(1)]);
}
