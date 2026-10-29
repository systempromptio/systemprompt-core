import { DocumentsElement } from "./documents-element.js";
import { listDocuments } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";
import { renderCrumbs, renderDocument, renderFolder, renderMenu, renderScopes } from "./documents-browser-view.js";

const rowExtensions = [];
const folderMenuItems = [];

/**
 * Adds to every row an installation renders: `extend({ kind, item })`, with
 * kind "folder" or "document", returns `{ attrs, html }` or nothing. `attrs`
 * keys must start with `data-` and are escaped here; `html` is the
 * extension's own markup, placed in the row's `.sp-doc-browser__ext` slot,
 * and the extension escapes what it interpolates.
 */
export function registerRowExtension(extend) {
  rowExtensions.push(extend);
}

/**
 * Adds an entry to the folder menu (the row's "More" button, or a right
 * click on the row): `{ id, label, run({ folder, browser }) }`.
 */
export function registerFolderMenuItem(item) {
  folderMenuItems.push(item);
}

export class SpDocumentBrowser extends DocumentsElement {
  onConnect() {
    this.scope = this.getAttribute("scope") ?? "mine";
    this.filter = "";
    this.path = [];
    this.listing = { folders: [], documents: [] };
    this.error = null;
    this.on("action:scope", (el) => this.go({ scope: el.dataset.scope, path: [] }));
    this.on("action:folder", (el) => this.go({ path: [...this.path, { id: el.dataset.id, name: el.dataset.name }] }));
    this.on("action:crumb", (el) => this.go({ path: this.path.slice(0, Number(el.dataset.depth)) }));
    this.on("action:open", (el) => this.dispatchEvent(new CustomEvent("sp-document-open", {
      bubbles: true,
      detail: { id: el.dataset.id, format: el.dataset.format, name: el.dataset.name },
    })));
    this.menuFor = null;
    this.on("action:folder-menu", (el) => this.toggleMenu(el.dataset.id));
    this.on("action:folder-menu-item", (el) => this.runMenuItem(el.dataset.item));
    this.addEventListener("contextmenu", (event) => {
      const row = event.target.closest("[data-folder-row]");
      if (row && folderMenuItems.length) {
        event.preventDefault();
        this.toggleMenu(row.dataset.folderRow);
      }
    });
    this.on("input:filter", (el) => { this.filter = el.value; this.invalidate(); });
    this.on("submit:search", () => this.load());
    this.onDocumentEvent((name) => { if (name === "document.revision.created") { this.load(); } });
    this.load();
  }

  toggleMenu(id) {
    this.menuFor = this.menuFor === id ? null : id;
    this.invalidate();
  }

  runMenuItem(itemId) {
    const folder = (this.listing.folders ?? []).find((f) => f.id === this.menuFor);
    const item = folderMenuItems.find((entry) => entry.id === itemId);
    this.menuFor = null;
    this.invalidate();
    if (folder && item) { item.run({ folder, browser: this }); }
  }

  go({ scope = this.scope, path = this.path }) {
    this.scope = scope;
    this.path = path;
    this.load();
  }

  async load() {
    const folder = this.path.at(-1)?.id;
    try {
      this.listing = await listDocuments({ scope: this.scope, folder, query: this.filter.trim() });
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    this.invalidate();
  }

  matches(name) {
    const needle = this.filter.trim().toLowerCase();
    return !needle || String(name).toLowerCase().includes(needle);
  }

  render() {
    const folders = (this.listing.folders ?? []).filter((f) => this.matches(f.name));
    const documents = (this.listing.documents ?? []).filter((d) => this.matches(d.name));
    const empty = folders.length + documents.length === 0 ? `<li class="sp-doc-browser__empty">No documents</li>` : "";
    const error = this.error ? `<p class="sp-doc-browser__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    return `<section class="sp-doc-browser">
      <div class="sp-doc-browser__scopes" role="group" aria-label="Scope">${renderScopes(this.scope)}</div>
      <form class="sp-doc-browser__search" role="search" data-submit="search">
        <input class="sp-doc-browser__filter" type="search" aria-label="Filter by name" data-input="filter"${attr("value", this.filter)}>
      </form>
      ${renderCrumbs(this.path)}${error}
      <ul class="sp-doc-browser__list">${folders.map((f) => renderFolder(rowExtensions, f, renderMenu(folderMenuItems, f, this.menuFor === f.id))).join("")}${documents.map((d) => renderDocument(rowExtensions, d)).join("")}${empty}</ul>
    </section>`;
  }
}

customElements.define("sp-document-browser", SpDocumentBrowser);
