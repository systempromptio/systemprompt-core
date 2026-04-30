import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";

export class SpToast extends BridgeElement {
  static properties = {
    message: { state: true },
    kind: { state: true },
    visible: { state: true },
  };

  constructor() {
    super();
    this.message = "";
    this.kind = "info";
    this.visible = false;
    this._timer = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-toast");
    this.setAttribute("role", "status");
    this.setAttribute("aria-live", "assertive");
    this.setAttribute("aria-atomic", "true");
    this.bridgeSubscribe("error", (err) => {
      const msg = (err && err.message) || "error";
      this.show(msg, "error", 8000);
    });
  }

  disconnectedCallback() {
    if (this._timer) { clearTimeout(this._timer); this._timer = null; }
    super.disconnectedCallback();
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

  updated() {
    this.hidden = !this.visible;
    if (this.visible) {
      this.dataset.kind = this.kind;
    }
  }

  render() {
    if (!this.visible) { return nothing; }
    return html`
      <span class="sp-toast__msg">${this.message}</span>
      <button class="sp-toast__close" type="button" aria-label="Dismiss" @click=${() => this.hide()}>×</button>
    `;
  }
}

customElements.define("sp-toast", SpToast);
