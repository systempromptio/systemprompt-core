import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { MKT_KINDS, createListingFetcher } from "/assets/js/services/marketplace-service.js";
import "/assets/js/components/sp-marketplace-list.js";
import "/assets/js/components/sp-marketplace-detail.js";

const KIND_LABEL = {
  plugins: "Plugins",
  skills: "Skills",
  hooks: "Hooks",
  mcp: "MCP servers",
  agents: "Agents",
};

const KIND_GLYPH = {
  plugins: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M18 13v4a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h6"/><path d="M9 13h6"/><path d="M9 17h4"/></svg>`,
  skills: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v4"/><path d="M12 18v4"/><path d="M4.93 4.93l2.83 2.83"/><path d="M16.24 16.24l2.83 2.83"/><path d="M2 12h4"/><path d="M18 12h4"/><path d="M4.93 19.07l2.83-2.83"/><path d="M16.24 7.76l2.83-2.83"/></svg>`,
  hooks: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v8"/><path d="M12 12a4 4 0 1 0 4 4"/></svg>`,
  mcp: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01"/><path d="M7 17h.01"/></svg>`,
  agents: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
};

function badgeView(snap) {
  if (!snap.signed_in) { return { text: t("marketplace-badge-signin") || "sign in", cls: "sp-badge--warn" }; }
  if (snap.sync_in_flight) { return { text: t("marketplace-badge-syncing") || "syncing", cls: "sp-badge--warn" }; }
  if (snap.last_sync_summary) { return { text: t("marketplace-badge-synced") || "synced", cls: "sp-badge--ok" }; }
  return { text: t("marketplace-badge-never") || "never synced", cls: "sp-badge--muted" };
}

export class SpMarketplace extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.listing = null;
    this.kind = "plugins";
    this.selectedId = null;
    this.search = "";
    this._fetcher = createListingFetcher();
    this.registerAction("select-kind", (trigger) => {
      this.kind = trigger.dataset.kind;
      this.selectedId = null;
    });
    this.registerAction("sync", async () => {
      try { await bridge.sync(); } catch (e) { console.warn(e); }
    });
    this.registerAction("validate", async () => {
      try { await bridge.validate(); } catch (e) { console.warn(e); }
    });
    this.registerAction("open-folder", async () => {
      try { await bridge.openConfigFolder(); } catch (e) { console.warn(e); }
    });
    this.registerAction("input:search", (trigger) => {
      this.search = trigger.value || "";
      this.selectedId = null;
    });
    this.addEventListener("mkt-select", (e) => { this.selectedId = e.detail.id; });
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; this._maybeFetch(s); }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; this._maybeFetch(s); });
  }

  async _maybeFetch(snap) {
    const next = await this._fetcher.maybeFetch(snap);
    if (next) { this.listing = next; }
  }

  afterRender() {
    const list = this.querySelector("sp-marketplace-list");
    const detail = this.querySelector("sp-marketplace-detail");
    const items = (this.listing && this.listing[this.kind]) || [];
    if (list) {
      list.items = items;
      list.search = this.search;
      list.selectedId = this.selectedId;
      list.kind = this.kind;
    }
    if (detail) {
      detail.selected = items.find((it) => it.id === this.selectedId) || null;
      detail.kind = this.kind;
    }
    const input = this.querySelector("#mkt-search");
    if (input && input.value !== this.search) { input.value = this.search; }
  }

  render() {
    const snap = this.snapshot || {};
    const badge = badgeView(snap);
    const counts = MKT_KINDS.reduce((acc, k) => {
      acc[k] = (this.listing && this.listing[k] || []).length;
      return acc;
    }, {});
    const cats = MKT_KINDS.map((k) => `
      <li class="sp-mkt-cat" data-kind="${k}" role="tab" aria-selected="${this.kind === k ? "true" : "false"}" tabindex="0" data-action="select-kind">
        <span class="sp-mkt-cat__glyph" aria-hidden="true">${KIND_GLYPH[k]}</span>
        <span class="sp-mkt-cat__name" data-l10n-id="marketplace-cat-${k}">${escapeHtml(KIND_LABEL[k])}</span>
        <span class="sp-mkt-cat__count ${counts[k] === 0 ? "is-zero" : ""}">${counts[k]}</span>
      </li>
    `).join("");
    const syncDisabled = snap.sync_in_flight || !snap.signed_in;
    const mktState = snap.last_sync_summary ? "ok" : "never";
    return `
      <header class="sp-tab__header">
        <h1 data-l10n-id="marketplace-heading">Marketplace</h1>
        <span class="sp-badge ${badge.cls}">${escapeHtml(badge.text)}</span>
      </header>
      <div class="sp-mkt">
        <ul class="sp-mkt-cats" role="tablist" aria-label="Marketplace categories">
          <li class="sp-mkt-cats__label" aria-hidden="true" data-l10n-id="marketplace-categories">Categories</li>
          ${cats}
        </ul>
        <div class="sp-mkt-list">
          <label class="sp-mkt-search__wrap">
            <svg class="sp-mkt-search__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
            <input id="mkt-search" class="sp-mkt-search" type="search" placeholder="Search…" data-l10n-placeholder="marketplace-search-placeholder" autocomplete="off" spellcheck="false" data-input="search" />
            <span class="sp-mkt-search__kbd" aria-hidden="true">⌘F</span>
          </label>
          <sp-marketplace-list></sp-marketplace-list>
        </div>
        <sp-marketplace-detail></sp-marketplace-detail>
      </div>
      <footer class="sp-mkt-actions">
        <button class="sp-btn-primary" type="button" data-l10n-id="sync-button" ${syncDisabled ? "disabled" : ""} data-action="sync">Sync now</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-validate" data-action="validate">Validate</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-open-folder" data-action="open-folder">Open folder</button>
        <span class="sp-mkt-actions__meta" data-state="${mktState}" title="${escapeHtml(snap.last_sync_summary || "—")}">
          <span class="sp-dot" aria-hidden="true"></span>
          <span data-l10n-id="last-sync-never">${escapeHtml(snap.last_sync_summary || "never synced")}</span>
        </span>
      </footer>
    `;
  }
}

reactive(SpMarketplace.prototype, ["snapshot", "listing", "kind", "selectedId", "search"]);
customElements.define("sp-marketplace", SpMarketplace);
