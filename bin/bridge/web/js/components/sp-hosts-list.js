import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t, brandArgs } from "/assets/js/i18n.js";
import { isListed } from "/assets/js/utils/verdict.js";
import { agentBanner, runBannerFix } from "/assets/js/components/agents-banners.js";
import { sameHost } from "/assets/js/utils/host-diff.js";
import "/assets/js/components/sp-agent-row.js";
import "/assets/js/components/sp-agent-drawer.js";
import "/assets/js/components/sp-overall-badge.js";
import "/assets/js/components/sp-agents-status.js";
import "/assets/js/components/sp-sync-banner.js";

export class SpHostsList extends SpElement {
  constructor() {
    super();
    this.hostsById = new Map();
    this.order = [];
    this.snapshot = null;
    this.gated = false;
    this.bannerBusy = false;

    this.registerAction("reverify-all", async () => {
      // Only agents with something local to inspect: a sync-only agent has no
      // process or profile here, and probing it only logged "unknown host".
      const ids = this.order.filter((id) => this.hostsById.get(id)?.can_verify !== false);
      await Promise.allSettled([bridge.hostProxyProbe(), ...ids.map((id) => bridge.hostProbe(id))]);
    });
    this.registerAction("banner-fix", async (trigger) => {
      if (this.bannerBusy) { return; }
      this.bannerBusy = true;
      try { await runBannerFix(this.hostsById.get(trigger.dataset.hostId), trigger.dataset.kind); }
      finally { this.bannerBusy = false; }
    });
    this.registerAction("add-agent", (trigger) => {
      const drawer = this._drawer();
      if (drawer) { drawer.open("add", null, trigger); }
    });
    // Fired by sp-agent-row's main button. The row's own click handler ignores
    // this action name and this one ignores the row's, so the single bubbling
    // click is never handled twice.
    this.registerAction("select-agent", (trigger) => {
      const drawer = this._drawer();
      if (drawer) { drawer.open("detail", trigger.dataset.hostId, trigger); }
    });
  }

  // The drawer is a body-level singleton (index.html), not a child: a fixed
  // element inside `.sp-content` (a size and masked scroll container) is
  // clipped out of the paint entirely, and a child would be rebuilt per render.
  _drawer() {
    return document.querySelector("sp-agent-drawer");
  }

  onConnect() {
    this.useSnapshot((s) => this._applyFullSnapshot(s));
    this.bridgeSubscribe("host.changed", (host) => this._applyHostDelta(host));
  }

  _applyFullSnapshot(snap) {
    if (!snap) { return; }
    this.snapshot = snap;
    this.gated = !!snap.hosts_gated;
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

  _hosts() {
    return this.order.map((id) => this.hostsById.get(id)).filter(Boolean);
  }


  render() {
    // Every installed agent is listed whatever its state, plus leftovers.
    const listed = this._hosts().filter(isListed);
    return `${this._renderHeader()}<sp-agents-status></sp-agents-status><sp-sync-banner></sp-sync-banner>${agentBanner(listed, this.bannerBusy)}${this._renderBody(listed)}`;
  }

  _renderHeader() {
    return `
      <header class="sp-tab__header sp-hosts__header">
        <div class="sp-hosts__header-meta">
          <h1 data-l10n-id="agents-heading">${escapeHtml(t("agents-heading") || "Agents")}</h1>
          <sp-overall-badge scope="set-up"></sp-overall-badge>
        </div>
        <div class="sp-hosts__header-actions">
          <button class="sp-btn-primary" type="button" data-action="add-agent">
            <span class="sp-hosts__plus" aria-hidden="true">+</span> ${escapeHtml(t("agents-action-add") || "Add agent")}
          </button>
          <button class="sp-btn-ghost" type="button" data-action="reverify-all">${escapeHtml(t("agents-action-reverify-all") || "Re-verify")}</button>
        </div>
        <p class="sp-tab__lede sp-u-muted">${escapeHtml(t("agents-lede") || `Every agent you add here runs through ${brandArgs().vendor}'s local proxy, so each request it makes is governed and logged. Any number of them can run at once.`)}</p>
      </header>
    `;
  }

  _renderBody(listed) {
    if (listed.length === 0) {
      return `<div class="sp-hosts__empty">
        <p class="sp-hosts__empty-title">${escapeHtml(t("agents-empty-title") || "No agents set up yet")}</p>
        <p class="sp-u-muted">${escapeHtml(t("agents-empty-body") || `Add a coding agent to route it through ${brandArgs().vendor}, so every request it makes is governed and logged.`)}</p>
        <button class="sp-btn-primary" type="button" data-action="add-agent">
          <span class="sp-hosts__plus" aria-hidden="true">+</span> ${escapeHtml(t("agents-action-add") || "Add agent")}
        </button>
      </div>`;
    }
    // data-key lets the reconciler reuse the same row element per host across
    // renders instead of rebuilding it (and its whole subtree) each time.
    return listed.map((h) => `<sp-agent-row data-key="${escapeHtml(h.id)}" data-host-id="${escapeHtml(h.id)}"></sp-agent-row>`).join("");
  }

  afterRender() {
    for (const el of this.querySelectorAll("sp-agent-row")) {
      const host = this.hostsById.get(el.dataset.hostId);
      if (!host) { continue; }
      // The reactive setters compare by identity, and these are fresh objects
      // every time — so gate on content to keep an unchanged row from
      // re-rendering.
      if (!sameHost(el.host, host)) { el.host = host; }
      if (!sameHost(el.snapshot, this.snapshot)) { el.snapshot = this.snapshot; }
    }
    const drawer = this._drawer();
    if (drawer) {
      const hosts = this._hosts();
      if (!sameHost(drawer.hosts, hosts)) { drawer.hosts = hosts; }
      if (!sameHost(drawer.snapshot, this.snapshot)) { drawer.snapshot = this.snapshot; }
      drawer.gated = this.gated;
    }
  }
}

reactive(SpHostsList.prototype, ["snapshot", "gated", "bannerBusy"]);
customElements.define("sp-hosts-list", SpHostsList);
