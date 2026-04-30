import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-host-card.js";

export class SpHostsList extends BridgeElement {
  static properties = {
    hostsById: { state: true },
    order: { state: true },
    snapshot: { state: true },
  };

  constructor() {
    super();
    this.hostsById = new Map();
    this.order = [];
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
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
  }

  render() {
    if (this.order.length === 0) {
      return html`<div class="sp-u-muted sp-host-list__empty">${t("hosts-empty") || "No host apps detected."}</div>`;
    }
    return html`${this.order.map((id) => {
      const host = this.hostsById.get(id);
      if (!host) { return null; }
      return html`<sp-host-card .host=${host} .snapshot=${this.snapshot} data-host-id=${id}></sp-host-card>`;
    })}`;
  }
}

customElements.define("sp-hosts-list", SpHostsList);
