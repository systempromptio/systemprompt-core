import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { actionFor } from "/assets/js/components/toast-actions.js";

// Every failing handler on the Rust side emits on the `error` channel *and*
// rejects the request (`finish()` in src/gui/handlers/*). The front end now
// reports its own rejections, so without this window a single back-end failure
// would say the same thing twice.
const DEDUPE_MS = 2000;


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
      const clears = Boolean(this.action && this.action.clearsOnSignIn);
      if (this.visible && clears && identity && identity.user_id) { this.hide(); }
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
    await action.run();
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
