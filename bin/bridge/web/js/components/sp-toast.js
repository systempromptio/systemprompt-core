import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";

export class SpToast extends SpElement {
  constructor() {
    super();
    this.message = "";
    this.kind = "info";
    this.visible = false;
    this._timer = null;
    this.offerReauth = false;
    this.registerAction("dismiss", () => this.hide());
    this.registerAction("reauth", () => {
      this.hide();
      document.body.classList.add("is-setup-mode");
    });
  }

  onConnect() {
    this.classList.add("sp-toast");
    this.setAttribute("role", "status");
    this.setAttribute("aria-live", "assertive");
    this.setAttribute("aria-atomic", "true");
    this.bridgeSubscribe("error", (err) => {
      const msg = (err && err.message) || "error";
      // A rejected credential is the one error the user can act on from here;
      // every other toast is informational.
      this.offerReauth = err && err.code === "unauthorized";
      this.show(msg, "error", this.offerReauth ? 0 : 8000);
    });
  }

  onDisconnect() {
    if (this._timer) { clearTimeout(this._timer); this._timer = null; }
  }

  show(message, kind = "info", durationMs = 6000) {
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
    this.hidden = !this.visible;
    if (this.visible) {
      this.dataset.kind = this.kind;
    } else {
      delete this.dataset.kind;
    }
  }

  render() {
    if (!this.visible) { return ""; }
    const reauth = this.offerReauth
      ? `<button class="sp-toast__action" type="button" data-action="reauth">${escapeHtml(t("sync-reauthenticate"))}</button>`
      : "";
    return `
      <span class="sp-toast__msg">${escapeHtml(this.message)}</span>
      ${reauth}
      <button class="sp-toast__close" type="button" aria-label="Dismiss" data-action="dismiss">×</button>
    `;
  }
}

reactive(SpToast.prototype, ["message", "kind", "visible", "offerReauth"]);
customElements.define("sp-toast", SpToast);
