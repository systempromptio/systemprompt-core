import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";

export const TAB_LABELS = {
  marketplace: "Marketplace",
  agents: "Agents",
  status: "Status",
  settings: "Settings",
};

export class SpCrumb extends SpElement {
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

  onConnect() {
    this.classList.add("sp-crumb");
    this.setAttribute("aria-label", "Breadcrumb");
    this._unsub = onBridgeEvent("crumb:set", this._onSet);
  }

  onDisconnect() {
    if (this._unsub) { this._unsub(); this._unsub = null; }
  }

  afterRender() {
    this.dataset.changing = this.changing ? "true" : "false";
  }

  render() {
    return `
      <span class="sp-crumb__sep" aria-hidden="true">/</span>
      <span class="sp-crumb__current">${escapeHtml(this.label)}</span>
    `;
  }
}

reactive(SpCrumb.prototype, ["label", "changing"]);
customElements.define("sp-crumb", SpCrumb);
