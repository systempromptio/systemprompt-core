import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";

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

export class SpMarketplaceList extends SpElement {
  constructor() {
    super();
    this.items = [];
    this.search = "";
    this.selectedId = null;
    this.kind = "plugins";
    this.registerAction("select-item", (trigger) => {
      this.dispatchEvent(new CustomEvent("mkt-select", {
        detail: { id: trigger.dataset.id }, bubbles: true, composed: true,
      }));
    });
  }

  render() {
    const items = filterItems(this.items || [], this.search);
    if (items.length === 0) {
      const title = this.search ? "No matches" : (KIND_EMPTY_TITLE[this.kind] || "Nothing here yet");
      return `<ul class="sp-mkt-items"><li class="sp-mkt-empty--with-sync"><span class="sp-mkt-empty__title">${escapeHtml(title)}</span></li></ul>`;
    }
    return `<ul class="sp-mkt-items">${items.map((it, i) => {
      const selected = it.id === this.selectedId ? "true" : "false";
      const sourceChip = it.source
        ? `<span class="sp-mkt-chip" data-tone="${it.source === "local" ? "" : "accent"}">${escapeHtml(it.source)}</span>`
        : "";
      const meta = it.summary ? `<div class="sp-mkt-item__meta">${escapeHtml(it.summary)}</div>` : "";
      return `
        <li class="sp-mkt-item" data-id="${escapeHtml(it.id)}" aria-selected="${selected}" style="--sp-mkt-item-i: ${Math.min(i, 8)}" data-action="select-item">
          <div class="sp-mkt-item__row">
            <span class="sp-mkt-item__name">${escapeHtml(it.name || it.id)}</span>
            ${sourceChip}
          </div>
          ${meta}
        </li>
      `;
    }).join("")}</ul>`;
  }
}

reactive(SpMarketplaceList.prototype, ["items", "search", "selectedId", "kind"]);
customElements.define("sp-marketplace-list", SpMarketplaceList);
