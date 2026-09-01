import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { handleRovingKey } from "/assets/js/utils/roving.js";
import { shortcut } from "/assets/js/utils/rail-tabs.js";
import { MKT_KINDS, createListingFetcher } from "/assets/js/services/marketplace-service.js";
import { runAction } from "/assets/js/utils/action.js";
import "/assets/js/components/sp-marketplace-list.js";
import "/assets/js/components/sp-marketplace-detail.js";

const KIND_LABEL = {
  plugins: "Plugins",
  skills: "Skills",
  hooks: "Hooks",
  mcp: "MCP servers",
  agents: "Agents",
  artifacts: "Artifacts",
};

const KIND_L10N = {
  plugins: "marketplace-cat-plugins",
  skills: "marketplace-cat-skills",
  hooks: "marketplace-cat-hooks",
  mcp: "marketplace-cat-mcp",
  agents: "marketplace-cat-agents",
  artifacts: "marketplace-cat-artifacts",
};

const KIND_GLYPH = {
  plugins: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M18 13v4a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h6"/><path d="M9 13h6"/><path d="M9 17h4"/></svg>`,
  skills: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v4"/><path d="M12 18v4"/><path d="M4.93 4.93l2.83 2.83"/><path d="M16.24 16.24l2.83 2.83"/><path d="M2 12h4"/><path d="M18 12h4"/><path d="M4.93 19.07l2.83-2.83"/><path d="M16.24 7.76l2.83-2.83"/></svg>`,
  hooks: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v8"/><path d="M12 12a4 4 0 1 0 4 4"/></svg>`,
  mcp: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01"/><path d="M7 17h.01"/></svg>`,
  agents: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
  artifacts: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="3" width="16" height="18" rx="2"/><path d="M4 9h16"/><path d="M10 9v12"/></svg>`,
};

function diffSummary(diff) {
  if (!diff) { return ""; }
  const parts = [];
  if (diff.installed && diff.installed.length) { parts.push(`${diff.installed.length} added`); }
  if (diff.updated && diff.updated.length) { parts.push(`${diff.updated.length} updated`); }
  if (diff.removed && diff.removed.length) { parts.push(`${diff.removed.length} removed`); }
  if (parts.length === 0) { return "no changes since last sync"; }
  return parts.join(", ");
}

// The footer used to report the snapshot's counts while the list beside it
// reported the listing's, and the two disagreed on screen. Once the listing is
// loaded it is the only source; before that the snapshot's line is labelled as
// what it is.
function listingSummary(listing) {
  const parts = [];
  for (const k of MKT_KINDS) {
    const n = ((listing && listing[k]) || []).length;
    if (n) { parts.push(`${n} ${k}`); }
  }
  return parts.length ? parts.join(", ") : t("marketplace-empty-generic") || "Nothing here yet";
}

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
    this.listingState = "idle";
    this._fetcher = createListingFetcher(() => this._applyFetcher());
    this.registerAction("select-kind", (trigger) => {
      this.kind = trigger.dataset.kind;
      this.selectedId = null;
    });

    this._onCatKey = (e) => {
      const cats = Array.from(this.querySelectorAll(".sp-mkt-cat"));
      const cur = cats.findIndex((el) => el.dataset.kind === this.kind);
      handleRovingKey(e, cats, cur, {
        onMove: (target) => { this.kind = target.dataset.kind; this.selectedId = null; },
      });
    };
    this.registerAction("sync", (trigger) => runAction(trigger, {
      run: () => bridge.sync(),
      success: t("toast-sync-started") || "Sync started.",
      context: t("sync-button") || "Sync now",
    }));
    this.registerAction("validate", (trigger) => runAction(trigger, {
      run: () => bridge.validate(),
      success: t("toast-validate-ok") || "Configuration validated.",
      context: t("marketplace-action-validate") || "Re-check",
    }));
    this.registerAction("open-folder", (trigger) => runAction(trigger, {
      run: () => bridge.openConfigFolder(),
      success: t("toast-folder-opened") || "Opened the configuration folder.",
      context: t("marketplace-action-open-folder") || "Open folder",
    }));
    this.registerAction("input:search", (trigger) => {
      this.search = trigger.value || "";
      this.selectedId = null;
      this._pushChildState();
    });
    this.addEventListener("mkt-refresh", () => { this._fetcher.refresh(); });
    this.addEventListener("mkt-sync", (e) => {
      runAction(null, {
        run: () => bridge.sync(),
        success: t("toast-sync-started") || "Sync started.",
        context: t("sync-button") || "Sync now",
      });
      e.stopPropagation();
    });
    this.addEventListener("mkt-select", (e) => {
      this.selectedId = e.detail.id;
      this._pushChildState();
    });
    this.addEventListener("mkt-navigate", (e) => {
      const { kind, id } = e.detail;
      if (!this.listing || !(this.listing[kind] || []).some((it) => it.id === id)) { return; }
      this.kind = kind;
      this.selectedId = id;
      this.search = "";
      this._pushChildState();
    });
  }

  onConnect() {
    this.addEventListener("keydown", this._onCatKey);
    bridge.stateSnapshot().then((s) => { this.snapshot = s; this._maybeFetch(s); }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; this._maybeFetch(s); });
  }

  _maybeFetch(snap) {
    this._fetcher.maybeFetch(snap);
  }

  _applyFetcher() {
    this.listing = this._fetcher.listing;
    this.listingState = this._fetcher.state;
    this._pushChildState();
  }

  afterRender() {
    this._pushChildState();
  }

  _pushChildState() {
    const list = this.querySelector("sp-marketplace-list");
    const detail = this.querySelector("sp-marketplace-detail");
    const items = (this.listing && this.listing[this.kind]) || [];
    if (list) {
      list.items = items;
      list.search = this.search;
      list.selectedId = this.selectedId;
      list.kind = this.kind;
      list.state = this._fetcher.state;
      list.error = this._fetcher.error || "";
      list.reason = this._fetcher.reason || "";
      // id -> display name for the grouped list's section headers. Sourced
      // from the plugins category of the same snapshot, so a header can never
      // name a plugin this listing does not carry.
      list.pluginNames = ((this.listing && this.listing.plugins) || [])
        .reduce((acc, p) => { acc[p.id] = p.name || p.id; return acc; }, {});
    }
    if (detail) {
      detail.selected = items.find((it) => it.id === this.selectedId) || null;
      detail.kind = this.kind;
      detail.snapshot = this.snapshot;
      detail.knownIds = MKT_KINDS.reduce((acc, k) => {
        acc[k] = new Set(((this.listing && this.listing[k]) || []).map((it) => it.id));
        return acc;
      }, {});
    }
    const input = this.querySelector("#mkt-search");
    if (input && input.value !== this.search) { input.value = this.search; }
  }

  render() {
    const snap = this.snapshot || {};
    const badge = badgeView(snap);
    // A count of 0 next to a pane that has not loaded is a claim, not a fact.
    const loaded = this._fetcher.state === "ok";
    const counts = MKT_KINDS.reduce((acc, k) => {
      acc[k] = loaded ? (this.listing && this.listing[k] || []).length : null;
      return acc;
    }, {});
    // A real <button>: role="tab" on an <li> alongside an aria-hidden <li> label
    // is not a valid tablist, and every tab carried tabindex="0" with no key
    // handler, so the rail was five tab stops that did nothing.
    const cats = MKT_KINDS.map((k) => {
      const selected = this.kind === k;
      return `
      <li role="presentation">
        <button class="sp-mkt-cat" type="button" data-kind="${k}" role="tab" id="sp-mkt-cat-${k}" aria-controls="sp-mkt-items" aria-selected="${selected ? "true" : "false"}" tabindex="${selected ? "0" : "-1"}" data-action="select-kind">
          <span class="sp-mkt-cat__glyph" aria-hidden="true">${KIND_GLYPH[k]}</span>
          <span class="sp-mkt-cat__name" data-l10n-id="${KIND_L10N[k]}">${escapeHtml(KIND_LABEL[k])}</span>
          <span class="sp-mkt-cat__count ${counts[k] === 0 ? "is-zero" : ""}">${counts[k] === null ? "—" : counts[k]}</span>
        </button>
      </li>`;
    }).join("");
    const syncDisabled = snap.sync_in_flight || !snap.signed_in;
    const mktState = snap.last_sync_summary ? "ok" : "never";
    const diff = (this.listing && this.listing.last_sync_diff) || null;
    const diffLine = diff ? diffSummary(diff) : "";
    return `
      <header class="sp-tab__header">
        <h1 data-l10n-id="marketplace-heading">Marketplace</h1>
        <span class="sp-badge ${badge.cls}">${escapeHtml(badge.text)}</span>
      </header>
      <div class="sp-mkt">
        <ul class="sp-mkt-cats" role="tablist" aria-orientation="vertical" data-l10n-aria="marketplace-categories-aria" aria-label="Marketplace categories">
          <li class="sp-mkt-cats__label" aria-hidden="true" data-l10n-id="marketplace-categories">Categories</li>
          ${cats}
        </ul>
        <div class="sp-mkt-list">
          <label class="sp-mkt-search__wrap">
            <svg class="sp-mkt-search__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
            <input id="mkt-search" class="sp-mkt-search" type="search" placeholder="Search…" data-l10n-placeholder="marketplace-search-placeholder" autocomplete="off" spellcheck="false" data-input="search" />
            <span class="sp-mkt-search__kbd" aria-hidden="true">${escapeHtml(shortcut("F"))}</span>
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
          <span>${escapeHtml(loaded ? listingSummary(this.listing) : (snap.last_sync_summary || t("last-sync-never")))}</span>
          ${diffLine ? `<span class="sp-mkt-actions__diff">· ${escapeHtml(diffLine)}</span>` : ""}
        </span>
      </footer>
    `;
  }
}

reactive(SpMarketplace.prototype, ["snapshot", "listing", "kind", "listingState"]);
customElements.define("sp-marketplace", SpMarketplace);
