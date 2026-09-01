import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { handleRovingKey } from "/assets/js/utils/roving.js";
import { MKT_KINDS, createListingFetcher } from "/assets/js/services/marketplace-service.js";
import { runAction } from "/assets/js/utils/action.js";
import { renderMarketplaceHeader, renderMarketplaceCats, renderMarketplaceSearch, renderMarketplaceFooter } from "/assets/js/components/marketplace-shell.js";
import "/assets/js/components/sp-marketplace-list.js";
import "/assets/js/components/sp-marketplace-detail.js";

const SYNC_ACTION = () => ({
  run: () => bridge.sync(),
  success: t("toast-sync-started") || "Sync started.",
  context: t("sync-button") || "Sync now",
});

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
    this._onCatKey = (e) => {
      const cats = Array.from(this.querySelectorAll(".sp-mkt-cat"));
      const cur = cats.findIndex((el) => el.dataset.kind === this.kind);
      handleRovingKey(e, cats, cur, {
        onMove: (target) => { this.kind = target.dataset.kind; this.selectedId = null; },
      });
    };
    this._registerActions();
    this._bindChildEvents();
  }

  _registerActions() {
    this.registerAction("select-kind", (trigger) => {
      this.kind = trigger.dataset.kind;
      this.selectedId = null;
    });
    this.registerAction("sync", (trigger) => runAction(trigger, SYNC_ACTION()));
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
  }

  _bindChildEvents() {
    this.addEventListener("mkt-refresh", () => { this._fetcher.refresh(); });
    this.addEventListener("mkt-sync", (e) => {
      runAction(null, SYNC_ACTION());
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
    this.useSnapshot((s) => { this.snapshot = s; this._fetcher.maybeFetch(s); });
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
    // A count of 0 next to a pane that has not loaded is a claim, not a fact.
    const loaded = this._fetcher.state === "ok";
    const counts = MKT_KINDS.reduce((acc, k) => {
      acc[k] = loaded ? (this.listing && this.listing[k] || []).length : null;
      return acc;
    }, {});
    return `
      ${renderMarketplaceHeader(this.snapshot || {})}
      <div class="sp-mkt">
        ${renderMarketplaceCats(this, counts)}
        <div class="sp-mkt-list">
          ${renderMarketplaceSearch()}
          <sp-marketplace-list></sp-marketplace-list>
        </div>
        <sp-marketplace-detail></sp-marketplace-detail>
      </div>
      ${renderMarketplaceFooter(this, loaded)}
    `;
  }
}

reactive(SpMarketplace.prototype, ["snapshot", "listing", "kind", "listingState"]);
customElements.define("sp-marketplace", SpMarketplace);
