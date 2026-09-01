import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";

/** Re-check interval. Updates are a background nicety, not a live feed. */
export const UPDATE_RECHECK_MS = 6 * 60 * 60 * 1000;

/** The update phase carried on the state snapshot; see `UpdateUiState`. */
export function updateStateOf(snapshot) {
  return (snapshot && snapshot.update) || { phase: "unknown" };
}

/** True while the update is mid-flight and the trigger must not act again. */
export function isUpdateBusy(update) {
  return ["downloading", "installing"].includes(update.phase);
}

/**
 * Checks once the gateway probe has settled and we are actually signed in —
 * the endpoint is authenticated, so checking earlier just logs a 401. Cheap
 * to call on every snapshot: the timestamp guard on `component._checkedAt`
 * collapses the repeats.
 */
export function maybeCheckForUpdate(component) {
  const snap = component.snapshot;
  if (!snap || !snap.signed_in) { return; }
  const update = updateStateOf(snap);
  if (isUpdateBusy(update) || update.can_restart) { return; }
  const now = Date.now();
  if (component._checkedAt && now - component._checkedAt < UPDATE_RECHECK_MS) { return; }
  component._checkedAt = now;
  bridge.updateCheck().catch((e) => console.debug("update check failed", e));
}

export function installUpdate(component) {
  if (isUpdateBusy(updateStateOf(component.snapshot))) { return; }
  component.menuOpen = false;
  // Progress and failure both arrive on `state.changed`, so nothing to do
  // with the resolved value here.
  bridge.updateInstall().catch((e) => notifyErr(e, t("rail-profile-update-cta") || "Click here to update"));
}

export function restartForUpdate() {
  notifyOk(t("toast-update-restarting") || "Restarting to finish the update…");
  bridge.updateRestart().catch((e) => notifyErr(e, t("rail-profile-restart-cta") || "Restart to finish updating"));
}
