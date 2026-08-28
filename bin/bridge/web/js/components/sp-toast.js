import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { t } from "/assets/js/i18n.js";

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
    this.offerReauth = false;
    this.registerAction("dismiss", () => this.hide());
    this.registerAction("reauth", () => {
      this.hide();
      document.body.classList.add("is-setup-mode");
    });
  }

  onConnect() {
    this.classList.add("sp-toast");
    this.setAttribute("aria-atomic", "true");
    this.bridgeSubscribe("error", (err) => {
      const msg = (err && err.message) || "error";
      // A rejected credential is the one error the user can act on from here;
      // every other toast is informational.
      this.offerReauth = err && err.code === "unauthorized";
      // An error the user has to read must not time out (WCAG 2.2.1). The
      // `unauthorized` case already had no timeout; every error now behaves the
      // same way and is dismissed deliberately.
      this.show(msg, "error", 0);
    });
    this._unsubs.push(onBridgeEvent("sp:toast", (e) => {
      const d = (e && e.detail) || {};
      if (!d.message) { return; }
      this.offerReauth = false;
      this.show(d.message, d.kind || "info", d.durationMs === undefined ? 6000 : d.durationMs, d.key || d.message);
    }));
  }

  onDisconnect() {
    if (this._timer) { clearTimeout(this._timer); this._timer = null; }
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
    const reauth = this.offerReauth
      ? `<button class="sp-toast__action" type="button" data-action="reauth">${escapeHtml(t("sync-reauthenticate") || "Sign in again")}</button>`
      : "";
    return `
      <span class="sp-toast__msg">${escapeHtml(this.message)}</span>
      ${reauth}
      <button class="sp-toast__close" type="button" data-l10n-aria="toast-dismiss" aria-label="Dismiss" data-action="dismiss">×</button>
    `;
  }
}

reactive(SpToast.prototype, ["message", "kind", "visible", "offerReauth"]);
customElements.define("sp-toast", SpToast);
