import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-host-card.js";

export class SpHostsList extends SpElement {
  constructor() {
    super();
    this.hostsById = new Map();
    this.order = [];
    this.snapshot = null;
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => this._applyFullSnapshot(s)).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => this._applyFullSnapshot(s));
    this.bridgeSubscribe("host.changed", (host) => this._applyHostDelta(host));
  }

  _applyFullSnapshot(snap) {
    if (!snap) { return; }
    this.snapshot = snap;
    const list = snap.host_apps || [];
    const next = new Map();
    for (const h of list) { next.set(h.id, h); }
    this.hostsById = next;
    this.order = list.map((h) => h.id);
    this.invalidate();
  }

  _applyHostDelta(host) {
    if (!host || !host.id) { return; }
    const next = new Map(this.hostsById);
    next.set(host.id, host);
    this.hostsById = next;
    if (!this.order.includes(host.id)) {
      this.order = [...this.order, host.id];
    } else {
      this.order = [...this.order];
    }
    this.invalidate();
  }

  render() {
    if (this.order.length === 0) {
      return `<div class="sp-u-muted sp-host-list__empty">${escapeHtml(t("hosts-empty") || "No host apps detected.")}</div>`;
    }
    return this.order.map((id) => `<sp-host-card data-host-id="${escapeHtml(id)}"></sp-host-card>`).join("");
  }

  afterRender() {
    for (const card of this.querySelectorAll("sp-host-card")) {
      const id = card.dataset.hostId;
      const host = this.hostsById.get(id);
      if (host) {
        card.host = host;
        card.snapshot = this.snapshot;
      }
    }
  }
}

reactive(SpHostsList.prototype, ["snapshot"]);
customElements.define("sp-hosts-list", SpHostsList);
