import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { handleRovingKey, syncRoving } from "/assets/js/utils/roving.js";
import { changeBadge, filterItems, groupItems } from "/assets/js/components/marketplace-list-groups.js";
import { placeholderMarkup } from "/assets/js/components/marketplace-list-placeholder.js";

export class SpMarketplaceList extends SpElement {
  constructor() {
    super();
    this.items = [];
    this.pluginNames = {};
    this.search = "";
    this.selectedId = null;
    this.kind = "plugins";
    this.state = "idle";
    this.error = "";
    this.reason = "";
    this.registerAction("retry", () => {
      this.dispatchEvent(new CustomEvent("mkt-refresh", { bubbles: true, composed: true }));
    });
    this.registerAction("sync", () => {
      this.dispatchEvent(new CustomEvent("mkt-sync", { bubbles: true, composed: true }));
    });
    // Selection follows focus in this listbox: arrowing through the list is how
    // a keyboard user reads the detail pane, exactly as clicking is for a mouse.
    this._onKeydown = (e) => {
      const items = this._items();
      const cur = items.findIndex((el) => el.dataset.id === this._selectedId);
      handleRovingKey(e, items, cur, {
        onMove: (target) => this.dispatchEvent(new CustomEvent("mkt-select", {
          detail: { id: target.dataset.id }, bubbles: true, composed: true,
        })),
      });
    };

    this.registerAction("select-item", (trigger) => {
      this.dispatchEvent(new CustomEvent("mkt-select", {
        detail: { id: trigger.dataset.id }, bubbles: true, composed: true,
      }));
    });
  }

  onConnect() {
    this.addEventListener("keydown", this._onKeydown);
  }

  set selectedId(v) {
    if (this._selectedId === v) { return; }
    this._selectedId = v;
    this._syncSelection();
  }
  get selectedId() { return this._selectedId; }

  _items() {
    return Array.from(this.querySelectorAll(".sp-mkt-item"));
  }

  _syncSelection() {
    const id = this._selectedId;
    const items = this._items();
    let selected = -1;
    for (let i = 0; i < items.length; i += 1) {
      const isSelected = items[i].dataset.id === id;
      items[i].setAttribute("aria-selected", isSelected ? "true" : "false");
      if (isSelected) { selected = i; }
    }
    syncRoving(items, selected);
  }

  afterRender() { this._syncSelection(); }

  _placeholder() {
    return placeholderMarkup({
      state: this.state,
      error: this.error,
      search: this.search,
      kind: this.kind,
      reason: this.reason,
    });
  }

  _option(it, i, groupKey) {
    const sourceChip = it.source
      ? `<span class="sp-mkt-chip">${escapeHtml(it.source)}</span>`
      : "";
    const changeChip = changeBadge(it.change);
    const meta = it.summary ? `<div class="sp-mkt-item__meta">${escapeHtml(it.summary)}</div>` : "";
    const chipsRow = sourceChip ? `<div class="sp-mkt-item__chips">${sourceChip}</div>` : "";
    const removedClass = it.change === "removed" ? " sp-mkt-item--removed" : "";
    // The DOM id must stay unique: an item shipped by two plugins renders once
    // per group, and duplicate ids would break aria-activedescendant.
    const domId = groupKey
      ? `sp-mkt-item-${escapeHtml(groupKey)}-${escapeHtml(it.id)}`
      : `sp-mkt-item-${escapeHtml(it.id)}`;
    return `
      <li class="sp-mkt-item${removedClass}" role="option" id="${domId}" data-id="${escapeHtml(it.id)}" aria-selected="false" tabindex="-1" style="--sp-mkt-item-i: ${Math.min(i, 8)}" data-action="select-item">
        <div class="sp-mkt-item__row">
          <span class="sp-mkt-item__name">${escapeHtml(it.name || it.id)}</span>
          ${changeChip}
        </div>
        ${meta}
        ${chipsRow}
      </li>
    `;
  }

  render() {
    if (this.state !== "ok") { return this._placeholder(); }
    const items = filterItems(this.items || [], this.search);
    if (items.length === 0) { return this._placeholder(); }
    const open = `<ul class="sp-mkt-items" id="sp-mkt-items" role="listbox" data-l10n-aria="marketplace-items-aria" aria-label="Items in this category">`;
    const groups = groupItems(items, this.pluginNames || {});
    // One unnamed group means nothing here has an owner (plugins, MCP servers,
    // an install synced before ownership rode on the manifest). A single
    // "Ungrouped" header over the whole list is noise, so render flat.
    if (groups.length === 1 && groups[0].key === "") {
      return `${open}${items.map((it, i) => this._option(it, i, "")).join("")}</ul>`;
    }
    let i = 0;
    return `${open}${groups.map((g) => {
      const options = g.items.map((it) => this._option(it, i++, g.key)).join("");
      return `
        <li class="sp-mkt-group" role="group" aria-label="${escapeHtml(g.label)}">
          <div class="sp-mkt-group__header">
            <span class="sp-mkt-group__name">${escapeHtml(g.label)}</span>
            <span class="sp-mkt-group__count">${g.items.length}</span>
          </div>
          <ul class="sp-mkt-group__items" role="none">${options}</ul>
        </li>
      `;
    }).join("")}</ul>`;
  }
}

reactive(SpMarketplaceList.prototype, ["items", "pluginNames", "search", "kind", "state", "error", "reason"]);
customElements.define("sp-marketplace-list", SpMarketplaceList);
