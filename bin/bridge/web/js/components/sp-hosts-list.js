import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-host-card.js";
import "/assets/js/components/sp-overall-badge.js";
import "/assets/js/components/sp-agents-status.js";

// Timestamps advance on every probe whether or not anything changed; comparing
// them would make every tick look like a real update. Mirrors VOLATILE_KEYS on
// the Rust side (src/gui/emit.rs).
const VOLATILE_KEYS = new Set(["probed_at_unix", "last_probe_at_unix", "ttl_seconds"]);

function stable(value) {
  return JSON.stringify(value, (k, v) => (VOLATILE_KEYS.has(k) ? undefined : v));
}

function sameHost(a, b) {
  return a != null && b != null && stable(a) === stable(b);
}

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
    const known = this.order.includes(host.id);
    // A probe that found nothing new still emits host.changed. Re-rendering on
    // it is pure cost, so compare before invalidating.
    if (known && sameHost(this.hostsById.get(host.id), host)) { return; }
    const next = new Map(this.hostsById);
    next.set(host.id, host);
    this.hostsById = next;
    this.order = known ? [...this.order] : [...this.order, host.id];
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
    const statusStrip = `<sp-agents-status></sp-agents-status>`;
    if (this.order.length === 0) {
      return `${headerMarkup}${statusStrip}<div class="sp-u-muted sp-host-list__empty">${escapeHtml(t("hosts-empty") || "No host apps detected.")}</div>`;
    }
    // data-key lets the reconciler reuse the same card element per host across
    // renders instead of rebuilding it (and its whole subtree) each time.
    const cardsMarkup = this.order.map((id) => `<sp-host-card data-key="${escapeHtml(id)}" data-host-id="${escapeHtml(id)}"></sp-host-card>`).join("");
    return `${headerMarkup}${statusStrip}${cardsMarkup}`;
  }

  afterRender() {
    for (const card of this.querySelectorAll("sp-host-card")) {
      const id = card.dataset.hostId;
      const host = this.hostsById.get(id);
      if (!host) { continue; }
      // The reactive setters compare by identity, and these are fresh objects
      // every time — so gate on content to keep an unchanged card from
      // re-rendering.
      if (!sameHost(card.host, host)) { card.host = host; }
      if (!sameHost(card.snapshot, this.snapshot)) { card.snapshot = this.snapshot; }
    }
  }
}

reactive(SpHostsList.prototype, ["snapshot"]);
customElements.define("sp-hosts-list", SpHostsList);
