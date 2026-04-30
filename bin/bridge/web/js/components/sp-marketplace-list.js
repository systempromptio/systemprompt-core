import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";

const KIND_EMPTY_TITLE = {
  plugins: "No plugins yet",
  skills: "No skills yet",
  hooks: "No hooks yet",
  mcp: "No MCP servers yet",
  agents: "No agents yet",
};

function filterItems(items, search) {
  if (!search) { return items; }
  const q = search.toLowerCase();
  return items.filter((it) =>
    (it.name || "").toLowerCase().includes(q) ||
    (it.id || "").toLowerCase().includes(q) ||
    (it.summary || "").toLowerCase().includes(q));
}

export class SpMarketplaceList extends BridgeElement {
  static properties = {
    items: { attribute: false },
    search: { attribute: false },
    selectedId: { attribute: false },
    kind: { attribute: false },
  };

  constructor() {
    super();
    this.items = [];
    this.search = "";
    this.selectedId = null;
    this.kind = "plugins";
  }

  createRenderRoot() { return this; }

  _select(id) {
    this.dispatchEvent(new CustomEvent("mkt-select", { detail: { id }, bubbles: true, composed: true }));
  }

  render() {
    const items = filterItems(this.items || [], this.search);
    if (items.length === 0) {
      const title = this.search ? "No matches" : (KIND_EMPTY_TITLE[this.kind] || "Nothing here yet");
      return html`<ul class="sp-mkt-items"><li class="sp-mkt-empty--with-sync">
        <span class="sp-mkt-empty__title">${title}</span>
      </li></ul>`;
    }
    return html`<ul class="sp-mkt-items">${items.map((it, i) => html`
      <li class="sp-mkt-item" data-id=${it.id} aria-selected=${it.id === this.selectedId ? "true" : "false"} style=${`--sp-mkt-item-i: ${Math.min(i, 8)}`} @click=${(e) => { e.stopPropagation(); this._select(it.id); }}>
        <div class="sp-mkt-item__row">
          <span class="sp-mkt-item__name">${it.name || it.id}</span>
          ${it.source ? html`<span class="sp-mkt-chip" data-tone=${it.source === "local" ? "" : "accent"}>${it.source}</span>` : ""}
        </div>
        ${it.summary ? html`<div class="sp-mkt-item__meta">${it.summary}</div>` : ""}
      </li>
    `)}</ul>`;
  }
}

customElements.define("sp-marketplace-list", SpMarketplaceList);
