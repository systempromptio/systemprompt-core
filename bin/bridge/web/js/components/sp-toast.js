import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { repairHost } from "/assets/js/utils/host-actions.js";
import { notifyErr, notifyOk } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

// Every failing handler on the Rust side emits on the `error` channel *and*
// rejects the request (`finish()` in src/gui/handlers/*). The front end now
// reports its own rejections, so without this window a single back-end failure
// would say the same thing twice.
const DEDUPE_MS = 2000;

// The one action an error toast offers, decided from the error's code and
// scope. `elevation_required` names a state only an administrator can change,
// so the button runs the UAC-backed repair for that scope; `partial` means the
// sync finished with agents left behind, so the button opens the health table
// that lists them; `unauthorized` is answered by signing in again.
function actionFor(err) {
  if (!err) { return null; }
  if (err.code === "unauthorized") {
    return { kind: "reauth", label: t("sync-reauthenticate") || "Sign in again" };
  }
  if (err.code === "elevation_required") {
    const failures = (err.detail && err.detail.host_failures) || [];
    const host = failures.find((f) => f.needs_elevation);
    if (err.scope === "identity") {
      return { kind: "repair-config-dir", label: t("toast-action-repair-admin") || "Repair as administrator" };
    }
    if (host) {
      return { kind: "repair-host", hostId: host.host_id, label: t("toast-action-repair-admin") || "Repair as administrator" };
    }
  }
  if (err.code === "partial") {
    return { kind: "details", label: t("toast-action-details") || "Details" };
  }
  return null;
}

function gotoStatus() {
  const rail = document.querySelector("sp-rail");
  if (rail && typeof rail.activateTab === "function") {
    rail.activateTab("status");
  }
}

export class SpToast extends SpElement {
  constructor() {
    super();
    this.message = "";
    this.kind = "info";
    this.visible = false;
    this._timer = null;
    this._lastMessage = "";
    this._lastAt = 0;
    this.action = null;
    this.gatewayError = false;
    this.registerAction("dismiss", () => this.hide());
    this.registerAction("act", () => this._runAction());
  }

  onConnect() {
    this.classList.add("sp-toast");
    this.setAttribute("aria-atomic", "true");
    this.bridgeSubscribe("error", (err) => {
      const msg = (err && err.message) || "error";
      this.action = actionFor(err);
      // Why: a gateway that came back answers this toast as completely as a
      // fresh sign-in answers the credential one. Without this it outlived the
      // recovery and kept naming a host we no longer talk to.
      this.gatewayError = Boolean(err && err.scope === "gateway" && err.code === "unreachable");
      // An error the user has to read must not time out (WCAG 2.2.1). The
      // `unauthorized` case already had no timeout; every error now behaves the
      // same way and is dismissed deliberately.
      this.show(msg, "error", 0);
    });
    // A rejected credential has no timeout, so nothing but the user could
    // clear it — it outlived the purge and the fresh sign-in that answered it.
    // The moment the snapshot carries a verified identity, the answer has
    // arrived and the prompt is stale.
    this.useSnapshot((snap) => {
      const identity = (snap && snap.verified_identity) || null;
      const reauth = this.action && this.action.kind === "reauth";
      if (this.visible && reauth && identity && identity.user_id) { this.hide(); }
      const gateway = (snap && snap.gateway_status) || null;
      if (this.visible && this.gatewayError && gateway && gateway.tone === "ok") { this.hide(); }
    });
    this._unsubs.push(onBridgeEvent("sp:toast", (e) => {
      const d = (e && e.detail) || {};
      if (!d.message) { return; }
      this.action = null;
      this.gatewayError = false;
      this.show(d.message, d.kind || "info", d.durationMs === undefined ? 6000 : d.durationMs, d.key || d.message);
    }));
  }

  onDisconnect() {
    if (this._timer) { clearTimeout(this._timer); this._timer = null; }
  }

  async _runAction() {
    const action = this.action;
    this.hide();
    if (!action) { return; }
    switch (action.kind) {
      case "reauth":
        document.body.classList.add("is-setup-mode");
        return;
      case "details":
        gotoStatus();
        return;
      case "repair-config-dir":
        try {
          await bridge.configRepairDir();
          notifyOk(t("toast-config-dir-repaired") || "Configuration folder repaired. Sign in again.");
        } catch (e) {
          notifyErr(e, t("toast-action-repair-admin") || "Repair as administrator");
        }
        return;
      case "repair-host":
        try {
          await repairHost(action.hostId);
          notifyOk(t("toast-agent-repaired-short", { name: action.hostId }) || `${action.hostId} repaired.`);
        } catch (e) {
          notifyErr(e, t("toast-action-repair-admin") || "Repair as administrator");
        }
        return;
      default:
        return;
    }
  }

  show(message, kind = "info", durationMs = 6000, key = message) {
    const now = Date.now();
    if (key === this._lastMessage && now - this._lastAt < DEDUPE_MS) { return; }
    this._lastMessage = key;
    this._lastAt = now;
    this.message = message;
    this.kind = kind;
    this.visible = true;
    if (this._timer) { clearTimeout(this._timer); }
    if (durationMs > 0) {
      this._timer = setTimeout(() => this.hide(), durationMs);
    }
  }

  hide() {
    this.visible = false;
    this.gatewayError = false;
    if (this._timer) { clearTimeout(this._timer); this._timer = null; }
  }

  afterRender() {
    // The live region stays mounted and its contents change; toggling `hidden`
    // on the region host itself is unreliable for announcement. `role` follows
    // severity because role="status" with aria-live="assertive" is two
    // contradictory claims about the same node.
    this.setAttribute("role", this.kind === "error" ? "alert" : "status");
    this.dataset.empty = this.visible ? "false" : "true";
    if (this.visible) {
      this.dataset.kind = this.kind;
    } else {
      delete this.dataset.kind;
    }
  }

  render() {
    if (!this.visible) { return ""; }
    const action = this.action
      ? `<button class="sp-toast__action" type="button" data-action="act">${escapeHtml(this.action.label)}</button>`
      : "";
    return `
      <span class="sp-toast__msg">${escapeHtml(this.message)}</span>
      ${action}
      <button class="sp-toast__close" type="button" data-l10n-aria="toast-dismiss" aria-label="Dismiss" data-action="dismiss">×</button>
    `;
  }
}

reactive(SpToast.prototype, ["message", "kind", "visible", "action"]);
customElements.define("sp-toast", SpToast);
