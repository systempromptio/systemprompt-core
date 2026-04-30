import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";

export const TAB_LABELS = {
  marketplace: "Marketplace",
  agents: "Agents",
  status: "Status",
  settings: "Settings",
};

export class SpCrumb extends BridgeElement {
  static properties = { label: { state: true }, changing: { state: true } };

  constructor() {
    super();
    this.label = TAB_LABELS.marketplace;
    this.changing = false;
    this._onSet = (e) => {
      const name = e.detail && e.detail.name;
      const next = TAB_LABELS[name] || name || "";
      if (next === this.label) { return; }
      this.changing = true;
      setTimeout(() => {
        this.label = next;
        this.changing = false;
      }, 120);
    };
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-crumb");
    this.setAttribute("aria-label", "Breadcrumb");
    document.addEventListener("crumb:set", this._onSet);
  }

  disconnectedCallback() {
    document.removeEventListener("crumb:set", this._onSet);
    super.disconnectedCallback();
  }

  updated() {
    this.dataset.changing = this.changing ? "true" : "false";
  }

  render() {
    return html`
      <span class="sp-crumb__sep" aria-hidden="true">/</span>
      <span class="sp-crumb__current">${this.label}</span>
    `;
  }
}

customElements.define("sp-crumb", SpCrumb);
