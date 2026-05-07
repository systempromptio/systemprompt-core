import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-host-card.js";
import "/assets/js/components/sp-overall-badge.js";

export class SpHostsList extends SpElement {
  constructor() {
    super();
    this.hostsById = new Map();
    this.order = [];
    this.snapshot = null;
    this.registerAction("generate-all", async () => {
      await Promise.allSettled(this.order.map((id) => bridge.hostProfileGenerate(id)));
    });
    this.registerAction("install-all", async () => {
      const tasks = [];
      for (const id of this.order) {
        const host = this.hostsById.get(id);
        const path = host && host.last_generated_profile && host.last_generated_profile.path;
        if (path) { tasks.push(bridge.hostProfileInstall(id, path)); }
      }
      await Promise.allSettled(tasks);
    });
    this.registerAction("reverify-all", async () => {
      await Promise.allSettled(this.order.map((id) => bridge.hostProbe(id)));
    });
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => this._applyFullSnapshot(s)).catch((e) => console.warn("snapshot failed", e));
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
    const anyHasGenerated = this.order.some((id) => {
      const h = this.hostsById.get(id);
      return h && h.last_generated_profile && h.last_generated_profile.path;
    });
    const installDisabledAttr = anyHasGenerated ? "" : "disabled";
    const headerMarkup = `
      <header class="sp-hosts__header">
        <div class="sp-hosts__header-meta">
          <h1 data-l10n-id="agents-heading">${escapeHtml(t("agents-heading") || "Agents")}</h1>
          <sp-overall-badge></sp-overall-badge>
        </div>
        <div class="sp-hosts__header-actions">
          <button class="sp-btn-primary" type="button" data-action="generate-all">${escapeHtml(t("agents-action-generate-all") || "Generate")}</button>
          <button class="sp-btn-ghost" type="button" data-action="install-all" ${installDisabledAttr}>${escapeHtml(t("agents-action-install-all") || "Install")}</button>
          <button class="sp-btn-ghost" type="button" data-action="reverify-all">${escapeHtml(t("agents-action-reverify-all") || "Re-verify")}</button>
        </div>
      </header>
    `;
    if (this.order.length === 0) {
      return `${headerMarkup}<div class="sp-u-muted sp-host-list__empty">${escapeHtml(t("hosts-empty") || "No host apps detected.")}</div>`;
    }
    const cardsMarkup = this.order.map((id) => `<sp-host-card data-host-id="${escapeHtml(id)}"></sp-host-card>`).join("");
    return `${headerMarkup}${cardsMarkup}`;
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
