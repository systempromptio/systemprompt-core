import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";

export class SpToast extends SpElement {
  constructor() {
    super();
    this.message = "";
    this.kind = "info";
    this.visible = false;
    this._timer = null;
    this.registerAction("dismiss", () => this.hide());
  }

  onConnect() {
    this.classList.add("sp-toast");
    this.setAttribute("role", "status");
    this.setAttribute("aria-live", "assertive");
    this.setAttribute("aria-atomic", "true");
    this.bridgeSubscribe("error", (err) => {
      const msg = (err && err.message) || "error";
      this.show(msg, "error", 8000);
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
    if (this.visible) { this.dataset.kind = this.kind; }
  }

  render() {
    if (!this.visible) { return ""; }
    return `
      <span class="sp-toast__msg">${escapeHtml(this.message)}</span>
      <button class="sp-toast__close" type="button" aria-label="Dismiss" data-action="dismiss">×</button>
    `;
  }
}

reactive(SpToast.prototype, ["message", "kind", "visible"]);
customElements.define("sp-toast", SpToast);
