import { bridge } from "/assets/js/bridge.js";
import { repairHost } from "/assets/js/utils/host-actions.js";
import { notifyErr, notifyOk } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

function gotoStatus() {
  const rail = document.querySelector("sp-rail");
  if (rail && typeof rail.activateTab === "function") {
    rail.activateTab("status");
  }
}

function repairLabel() {
  return t("toast-action-repair-admin") || "Repair as administrator";
}

// Each action is what the button does, not a name the toast interprets:
// `run` performs it and `clearsOnSignIn` marks the one a fresh sign-in
// answers (the credential prompt outlives the sign-in otherwise).
function reauth() {
  return {
    label: t("sync-reauthenticate") || "Sign in again",
    clearsOnSignIn: true,
    run: () => { document.body.classList.add("is-setup-mode"); },
  };
}

function details() {
  return { label: t("toast-action-details") || "Details", run: gotoStatus };
}

function repairConfigDir() {
  return {
    label: repairLabel(),
    run: async () => {
      try {
        await bridge.configRepairDir();
        notifyOk(t("toast-config-dir-repaired") || "Configuration folder repaired. Sign in again.");
      } catch (e) {
        notifyErr(e, repairLabel());
      }
    },
  };
}

function repairHostAction(hostId) {
  return {
    label: t("toast-action-update-admin") || "Update as administrator",
    run: async () => {
      try {
        await repairHost(hostId);
        notifyOk(t("toast-agent-repaired-short", { name: hostId }) || `${hostId} repaired.`);
      } catch (e) {
        notifyErr(e, repairLabel());
      }
    },
  };
}

// The one action an error toast offers, decided from the error's code and
// scope. `elevation_required` names a state only an administrator can change,
// so the button runs the UAC-backed repair for that scope; `partial` means the
// sync finished with agents left behind, so the button opens the health table
// that lists them; `unauthorized` is answered by signing in again.
export function actionFor(err) {
  if (!err) { return null; }
  if (err.code === "unauthorized") {
    return reauth();
  }
  if (err.code === "elevation_required") {
    const failures = (err.detail && err.detail.host_failures) || [];
    const host = failures.find((f) => f.needs_elevation);
    if (err.scope === "identity") {
      return repairConfigDir();
    }
    if (host) {
      return repairHostAction(host.host_id);
    }
  }
  if (err.code === "partial") {
    return details();
  }
  return null;
}

