import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-marketplace-list.js";
import "/assets/js/components/sp-marketplace-detail.js";

const MKT_KINDS = ["plugins", "skills", "hooks", "mcp", "agents"];

const KIND_LABEL = {
  plugins: "Plugins",
  skills: "Skills",
  hooks: "Hooks",
  mcp: "MCP servers",
  agents: "Agents",
};

const KIND_GLYPH = {
  plugins: html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M18 13v4a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h6"/><path d="M9 13h6"/><path d="M9 17h4"/></svg>`,
  skills: html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v4"/><path d="M12 18v4"/><path d="M4.93 4.93l2.83 2.83"/><path d="M16.24 16.24l2.83 2.83"/><path d="M2 12h4"/><path d="M18 12h4"/><path d="M4.93 19.07l2.83-2.83"/><path d="M16.24 7.76l2.83-2.83"/></svg>`,
  hooks: html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v8"/><path d="M12 12a4 4 0 1 0 4 4"/></svg>`,
  mcp: html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01"/><path d="M7 17h.01"/></svg>`,
  agents: html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
};

function badgeView(snap) {
  if (!snap.signed_in) { return { text: t("marketplace-badge-signin") || "sign in", cls: "sp-badge--warn" }; }
  if (snap.sync_in_flight) { return { text: t("marketplace-badge-syncing") || "syncing", cls: "sp-badge--warn" }; }
  if (snap.last_sync_summary) { return { text: t("marketplace-badge-synced") || "synced", cls: "sp-badge--ok" }; }
  return { text: t("marketplace-badge-never") || "never synced", cls: "sp-badge--muted" };
}

export class SpMarketplace extends BridgeElement {
  static properties = {
    snapshot: { state: true },
    listing: { state: true },
    kind: { state: true },
    selectedId: { state: true },
    search: { state: true },
  };

  constructor() {
    super();
    this.snapshot = null;
    this.listing = null;
    this.kind = "plugins";
    this.selectedId = null;
    this.search = "";
    this._lastSyncSummary = null;
    this._inFlight = false;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    bridge.stateSnapshot().then((s) => { this.snapshot = s; this._maybeFetch(s); }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; this._maybeFetch(s); });
  }

  _maybeFetch(snap) {
    if (!snap || !snap.signed_in) { return; }
    if (snap.last_sync_summary !== this._lastSyncSummary || !this.listing) {
      this._lastSyncSummary = snap.last_sync_summary;
      this._fetchListing();
    }
  }

  async _fetchListing() {
    if (this._inFlight) { return; }
    this._inFlight = true;
    try {
      this.listing = await bridge.marketplaceList();
      this._broadcastCount();
    } catch (e) {
      console.error("marketplace list failed", e);
    } finally {
      this._inFlight = false;
    }
  }

  _broadcastCount() {
    if (!this.listing) { return; }
    let total = 0;
    for (const k of MKT_KINDS) { total += (this.listing[k] || []).length; }
    document.dispatchEvent(new CustomEvent("mkt:count", { detail: { total } }));
  }

  _selectKind(kind) { this.kind = kind; this.selectedId = null; }
  _selectItem(id) { this.selectedId = id; }

  async _onSync(e) {
    e.stopPropagation();
    try { await bridge.sync(); } catch (err) { console.warn(err); }
  }
  async _onValidate(e) {
    e.stopPropagation();
    try { await bridge.validate(); } catch (err) { console.warn(err); }
  }
  async _onOpenFolder(e) {
    e.stopPropagation();
    try { await bridge.openConfigFolder(); } catch (err) { console.warn(err); }
  }
  _onSearch(e) {
    this.search = e.target.value || "";
    this.selectedId = null;
  }

  render() {
    const snap = this.snapshot || {};
    const badge = badgeView(snap);
    const items = (this.listing && this.listing[this.kind]) || [];
    const counts = MKT_KINDS.reduce((acc, k) => {
      acc[k] = (this.listing && this.listing[k] || []).length;
      return acc;
    }, {});
    const selected = items.find((it) => it.id === this.selectedId) || null;
    const mktState = "never";
    return html`
      <header class="sp-tab__header">
        <h1 data-l10n-id="marketplace-heading">Marketplace</h1>
        <span class="sp-badge ${badge.cls}">${badge.text}</span>
      </header>
      <div class="sp-mkt">
        <ul class="sp-mkt-cats" role="tablist" aria-label="Marketplace categories">
          <li class="sp-mkt-cats__label" aria-hidden="true" data-l10n-id="marketplace-categories">Categories</li>
          ${MKT_KINDS.map((k) => html`
            <li class="sp-mkt-cat" data-kind=${k} role="tab" aria-selected=${this.kind === k ? "true" : "false"} tabindex="0" @click=${(e) => { e.stopPropagation(); this._selectKind(k); }}>
              <span class="sp-mkt-cat__glyph" aria-hidden="true">${KIND_GLYPH[k]}</span>
              <span class="sp-mkt-cat__name" data-l10n-id=${`marketplace-cat-${k}`}>${KIND_LABEL[k]}</span>
              <span class="sp-mkt-cat__count ${counts[k] === 0 ? "is-zero" : ""}">${counts[k]}</span>
            </li>
          `)}
        </ul>

        <div class="sp-mkt-list">
          <label class="sp-mkt-search__wrap">
            <svg class="sp-mkt-search__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
            <input id="mkt-search" class="sp-mkt-search" type="search" placeholder="Search…" data-l10n-placeholder="marketplace-search-placeholder" autocomplete="off" spellcheck="false" .value=${this.search} @input=${(e) => this._onSearch(e)} />
            <span class="sp-mkt-search__kbd" aria-hidden="true">⌘F</span>
          </label>
          <sp-marketplace-list .items=${items} .search=${this.search} .selectedId=${this.selectedId} .kind=${this.kind} @mkt-select=${(e) => this._selectItem(e.detail.id)}></sp-marketplace-list>
        </div>

        <sp-marketplace-detail .selected=${selected} .kind=${this.kind}></sp-marketplace-detail>
      </div>

      <footer class="sp-mkt-actions">
        <button class="sp-btn-primary" type="button" data-l10n-id="sync-button" ?disabled=${snap.sync_in_flight || !snap.signed_in} @click=${(e) => this._onSync(e)}>Sync now</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-validate" @click=${(e) => this._onValidate(e)}>Validate</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="marketplace-action-open-folder" @click=${(e) => this._onOpenFolder(e)}>Open folder</button>
        <span class="sp-mkt-actions__meta" data-state=${snap.last_sync_summary ? "ok" : mktState} title=${snap.last_sync_summary || "—"}>
          <span class="sp-dot" aria-hidden="true"></span>
          <span data-l10n-id="last-sync-never">${snap.last_sync_summary || "never synced"}</span>
        </span>
      </footer>
    `;
  }
}

customElements.define("sp-marketplace", SpMarketplace);
