import { bridge } from "/assets/js/bridge.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

// How long the wizard waits for every host to report before it says so. The
// gate had no timeout and no error path at all: one host that never reported
// left the user on onboarding indefinitely, with nothing on screen to say why.
const SETTLE_TIMEOUT_MS = 12_000;

export function clearSettleTimer(component) {
  if (component._settleTimer) { clearTimeout(component._settleTimer); component._settleTimer = null; }
}

export function armSettleTimer(component) {
  clearSettleTimer(component);
  component._settleTimer = setTimeout(() => {
    component._settleTimer = null;
    component.settleTimedOut = true;
  }, SETTLE_TIMEOUT_MS);
}

export function trackSettle(component, settled) {
  if (settled) {
    clearSettleTimer(component);
    component.settleTimedOut = false;
  } else if (!component._settleTimer && !component.settleTimedOut) {
    armSettleTimer(component);
  }
}

export function retrySettle(component) {
  component.settleTimedOut = false;
  armSettleTimer(component);
  bridge.gatewayProbe().catch((e) => notifyErr(e, t("setup-retry") || "Check again"));
  for (const h of (component.snapshot && component.snapshot.host_apps) || []) {
    bridge.hostProbe(h.id).catch((e) => console.warn(`host probe ${h.id} failed`, e));
  }
}
