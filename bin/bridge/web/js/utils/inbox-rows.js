// The inbox pane's row model, derived from the `InboxPayload` the bridge
// emits on `inbox.changed` and returns from `inbox.list`.

const ATTENTION = new Set(["human", "agent", "silent"]);
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export const EMPTY_INBOX = {
  unread: 0,
  unread_capped: false,
  has_older: false,
  items: [],
  can_start_session: false,
  available: true,
  last_error: null,
};

export function ageLabel(iso, nowMs) {
  const at = Date.parse(iso);
  if (Number.isNaN(at)) { return ""; }
  const delta = Math.max(0, nowMs - at);
  if (delta < MINUTE) { return "now"; }
  if (delta < HOUR) { return `${Math.floor(delta / MINUTE)}m`; }
  if (delta < DAY) { return `${Math.floor(delta / HOUR)}h`; }
  return `${Math.floor(delta / DAY)}d`;
}

export function unreadCount(payload) {
  if (payload?.available === false) { return 0; }
  const unread = payload?.unread;
  return Number.isInteger(unread) ? Math.max(0, unread) : 0;
}

// Why: the server stops counting unread at a cap and says so; the badge then
// reads as the cap with a plus rather than as an exact count.
export function unreadLabel(payload) {
  const unread = unreadCount(payload);
  if (unread === 0) { return null; }
  return payload?.unread_capped ? `${unread}+` : String(unread);
}

const toRow = (canStartSession, nowMs) => (item) => {
  const attention = ATTENTION.has(item.attention) ? item.attention : "silent";
  return {
    key: item.id,
    attention,
    title: item.title || "",
    viaAgent: item.via_agent ?? null,
    text: item.body || item.title || item.action?.prompt || "",
    createdAt: item.created_at,
    age: ageLabel(item.created_at, nowMs),
    count: item.coalesce_count > 1 ? item.coalesce_count : null,
    isUnread: item.read_at == null,
    canOpen: Boolean(item.action?.open_url),
    canEscalate: attention === "agent",
    canStartSession: Boolean(canStartSession),
  };
};

export function inboxRows(payload, nowMs = Date.now()) {
  const items = Array.isArray(payload?.items) ? payload.items : [];
  return [...items]
    .sort((a, b) => Date.parse(b.created_at) - Date.parse(a.created_at))
    .map(toRow(payload?.can_start_session, nowMs));
}
