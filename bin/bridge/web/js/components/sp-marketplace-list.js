import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { handleRovingKey, syncRoving } from "/assets/js/utils/roving.js";

const KIND_EMPTY_L10N = {
  plugins: "marketplace-empty-plugins",
  skills: "marketplace-empty-skills",
  hooks: "marketplace-empty-hooks",
  mcp: "marketplace-empty-mcp",
  agents: "marketplace-empty-agents",
  artifacts: "marketplace-empty-artifacts",
};

const KIND_EMPTY_TITLE = {
  plugins: "No plugins yet",
  skills: "No skills yet",
  hooks: "No hooks yet",
  mcp: "No MCP servers yet",
  agents: "No agents yet",
  artifacts: "No artifacts yet",
};

const CHANGE_L10N = {
  installed: "marketplace-change-installed",
  updated: "marketplace-change-updated",
  removed: "marketplace-change-removed",
};

const CHANGE_LABEL = {
  installed: "New",
  updated: "Updated",
  removed: "Removed",
};

function changeBadge(change) {
  if (!change || !CHANGE_LABEL[change]) { return ""; }
  const label = t(CHANGE_L10N[change]) || CHANGE_LABEL[change];
  return `<span class="sp-mkt-chip sp-mkt-chip--change" data-change-kind="${change}">${escapeHtml(label)}</span>`;
}

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

  // Loading, empty and broken used to render the same line. They are three
  // different situations and the only one the user can act on is the last two.
  _placeholder() {
    if (this.state === "loading" || this.state === "idle") {
      return `<ul class="sp-mkt-items" data-state="probing" aria-hidden="true">${
        [0, 1, 2, 3].map(() => `<li class="sp-mkt-item sp-mkt-item--skeleton" aria-hidden="true">
          <div class="sp-mkt-item__row"><span class="sp-mkt-item__name">&nbsp;</span></div>
          <div class="sp-mkt-item__meta">&nbsp;</div>
        </li>`).join("")
      }</ul>`;
    }
    if (this.state === "error") {
      return `<ul class="sp-mkt-items"><li class="sp-mkt-empty">
        <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-error-title") || "Could not load this list")}</span>
        <span class="sp-mkt-empty__sub">${escapeHtml(this.error || "")}</span>
        <button class="sp-btn-ghost" type="button" data-action="retry">${escapeHtml(t("marketplace-retry") || "Try again")}</button>
      </li></ul>`;
    }
    if (this.search) {
      return `<ul class="sp-mkt-items"><li class="sp-mkt-empty">
        <span class="sp-mkt-empty__title">${escapeHtml(t("marketplace-no-matches") || "No matches")}</span>
      </li></ul>`;
    }
    const neverSynced = this.reason === "never-synced";
    return `<ul class="sp-mkt-items"><li class="sp-mkt-empty--with-sync">
      <span class="sp-mkt-empty__title">${escapeHtml(t(KIND_EMPTY_L10N[this.kind]) || KIND_EMPTY_TITLE[this.kind]
        || t("marketplace-empty-generic") || "Nothing here yet")}</span>
      <span class="sp-mkt-empty__sub">${escapeHtml(
        neverSynced
          ? (t("marketplace-empty-never-synced") || "Sync to pull what your account already has.")
          : (t("marketplace-empty-synced") || "Your last sync did not include anything of this kind."))}</span>
      ${neverSynced
        ? `<button class="sp-btn-primary" type="button" data-action="sync">${escapeHtml(t("sync-button") || "Sync now")}</button>`
        : ""}
    </li></ul>`;
  }

  render() {
    if (this.state !== "ok") { return this._placeholder(); }
    const items = filterItems(this.items || [], this.search);
    if (items.length === 0) { return this._placeholder(); }
    return `<ul class="sp-mkt-items" id="sp-mkt-items" role="listbox" data-l10n-aria="marketplace-items-aria" aria-label="Items in this category">${items.map((it, i) => {
      const sourceChip = it.source
        ? `<span class="sp-mkt-chip">${escapeHtml(it.source)}</span>`
        : "";
      const changeChip = changeBadge(it.change);
      const meta = it.summary ? `<div class="sp-mkt-item__meta">${escapeHtml(it.summary)}</div>` : "";
      const chipsRow = sourceChip ? `<div class="sp-mkt-item__chips">${sourceChip}</div>` : "";
      const removedClass = it.change === "removed" ? " sp-mkt-item--removed" : "";
      return `
        <li class="sp-mkt-item${removedClass}" role="option" id="sp-mkt-item-${escapeHtml(it.id)}" data-id="${escapeHtml(it.id)}" aria-selected="false" tabindex="-1" style="--sp-mkt-item-i: ${Math.min(i, 8)}" data-action="select-item">
          <div class="sp-mkt-item__row">
            <span class="sp-mkt-item__name">${escapeHtml(it.name || it.id)}</span>
            ${changeChip}
          </div>
          ${meta}
          ${chipsRow}
        </li>
      `;
    }).join("")}</ul>`;
  }
}

reactive(SpMarketplaceList.prototype, ["items", "search", "kind", "state", "error", "reason"]);
customElements.define("sp-marketplace-list", SpMarketplaceList);
