// The one copy of the bridge's state snapshot in this page.
//
// Every component used to fetch its own snapshot on connect and re-fetch on
// the events it happened to know about — twenty-two copies of the same call,
// each with its own error handling, none of them agreeing on when to refresh.
// This module fetches once, refreshes on every channel the bridge re-emits
// state for, and hands the same object to every subscriber. Two panes reading
// the same snapshot cannot disagree about which snapshot they are reading.

import { bridge, subscribe } from "/assets/js/bridge.js";
import { notifyErr } from "/assets/js/utils/notify.js";

// Why: these channels announce a change the full snapshot already reflects;
// `state.changed` carries the snapshot itself.
const REFRESH_CHANNELS = ["gateway.changed", "proxy.changed", "mcp.changed"];

let snapshot = null;
let inflight = null;
const listeners = new Set();

function publish(next) {
  snapshot = next;
  for (const cb of listeners) {
    try { cb(snapshot); } catch (e) { console.error("state-store subscriber", e); }
  }
}

export function currentSnapshot() {
  return snapshot;
}

/** Fetch (or re-fetch) the snapshot; a failure is surfaced once, not per pane. */
export function refreshSnapshot() {
  if (inflight) { return inflight; }
  inflight = bridge.stateSnapshot()
    .then((s) => { publish(s); return s; })
    .catch((e) => { notifyErr(e, "state snapshot"); return snapshot; })
    .finally(() => { inflight = null; });
  return inflight;
}

/**
 * Subscribe to the snapshot. `cb` runs immediately if one is loaded, then on
 * every change. Returns the unsubscribe function.
 */
export function subscribeSnapshot(cb) {
  listeners.add(cb);
  if (snapshot) { cb(snapshot); } else { refreshSnapshot(); }
  return () => listeners.delete(cb);
}

subscribe("state.changed", publish);
for (const channel of REFRESH_CHANNELS) {
  subscribe(channel, () => { refreshSnapshot(); });
}
