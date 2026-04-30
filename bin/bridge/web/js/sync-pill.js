import { $ } from "./dom.js?t=__TOKEN__";
import { bridge } from "./bridge.js?t=__TOKEN__";
import { t } from "./i18n.js?t=__TOKEN__";

let cancelBound = false;

function classify(snap) {
  if (snap.sync_in_flight) {
    return { state: "running", text: t("sync-in-flight") || "syncing" };
  } else if (snap.gateway_status && snap.gateway_status.state === "unreachable") {
    return { state: "err", text: t("gateway-unreachable") || "offline" };
  } else if (snap.signed_in) {
    return { state: "ok", text: snap.last_sync_summary ? (t("sync-success") || "synced") : (t("ready") || "ready") };
  } else {
    return { state: "idle", text: t("gateway-not-signed-in") || "needs sign-in" };
  }
}

function bindCancel(button) {
  if (cancelBound) return;
  cancelBound = true;
  button.addEventListener("click", (ev) => {
    ev.preventDefault();
    bridge.cancel("sync").catch((e) => console.warn("cancel sync failed", e));
  });
}

export function renderSyncPill(snap) {
  const pill = $("sync-pill");
  if (!pill) return;
  const label = pill.querySelector(".sp-sync-pill__label");
  const cancel = pill.querySelector("#sync-cancel");
  const result = classify(snap);
  pill.dataset.state = result.state;
  if (label) {
    label.textContent = result.text;
  }
  if (cancel) {
    cancel.hidden = !snap.sync_in_flight;
    bindCancel(cancel);
  }
  pill.title = snap.last_sync_summary
    ? t("last-sync", { summary: snap.last_sync_summary }) || `Last sync: ${snap.last_sync_summary}`
    : (t("last-sync-never") || "No syncs yet");
}
