import { $ } from "./dom.js?t=__TOKEN__";

function classify(snap) {
  if (snap.sync_in_flight) {
    return { state: "running", text: "syncing" };
  } else if (snap.gateway_status && snap.gateway_status.state === "unreachable") {
    return { state: "err", text: "offline" };
  } else if (snap.signed_in) {
    return { state: "ok", text: snap.last_sync_summary ? "synced" : "ready" };
  } else {
    return { state: "idle", text: "needs sign-in" };
  }
}

export function renderSyncPill(snap) {
  const pill = $("sync-pill");
  if (pill) {
    const label = pill.querySelector(".sync-pill-label");
    const result = classify(snap);
    pill.dataset.state = result.state;
    if (label) {
      label.textContent = result.text;
    }
    pill.title = snap.last_sync_summary ? `Last sync: ${snap.last_sync_summary}` : "No syncs yet";
  }
}
