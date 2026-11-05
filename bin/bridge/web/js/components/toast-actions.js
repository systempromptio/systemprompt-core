import { bridge } from "/assets/js/bridge.js";
import { notifyErr, notifyOk } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

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

// The one action an error toast offers. A rejected credential is answered by
// signing in; a configuration folder another account owns by the UAC repair.
export function actionFor(err) {
  if (!err) { return null; }
  if (err.code === "unauthorized") { return reauth(); }
  if (err.code === "elevation_required" && err.scope === "identity") { return repairConfigDir(); }
  return null;
}
