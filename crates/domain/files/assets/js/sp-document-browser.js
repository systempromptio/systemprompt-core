import { DocumentsElement } from "./documents-element.js";
import { listDocuments } from "./documents-api.js";
import { attr, errorText, escapeHtml, formatBytes, formatTime } from "./documents-html.js";

const SCOPES = [["mine", "Mine"], ["shared", "Shared with me"], ["recent", "Recent"]];

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

function extensionsOf(kind, item) {
  let attrs = "";
  let html = "";
  for (const extend of rowExtensions) {
    const out = extend({ kind, item }) ?? {};
    for (const [name, value] of Object.entries(out.attrs ?? {})) {
      if (name.startsWith("data-")) { attrs += attr(name, value); }
    }
    html += out.html ?? "";
  }
  return { attrs, slot: html ? `<span class="sp-doc-browser__ext">${html}</span>` : "" };
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

  renderMenu(folder) {
    if (!folderMenuItems.length) { return ""; }
    const open = this.menuFor === folder.id;
    const toggle = `<button type="button" class="sp-doc-browser__more" data-action="folder-menu"${attr("data-id", folder.id)}${attr("aria-expanded", String(open))} aria-label="More">⋯</button>`;
    if (!open) { return toggle; }
    const items = folderMenuItems.map((item) => `<li role="none"><button type="button" role="menuitem" class="sp-doc-browser__menu-item" data-action="folder-menu-item"${attr("data-item", item.id)}>${escapeHtml(item.label)}</button></li>`).join("");
    return `${toggle}<ul class="sp-doc-browser__menu" role="menu">${items}</ul>`;
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

  renderScopes() {
    return SCOPES.map(([scope, label]) => `<button type="button" class="sp-doc-browser__scope${scope === this.scope ? " is-active" : ""}" data-action="scope"${attr("data-scope", scope)}${attr("aria-pressed", String(scope === this.scope))}>${escapeHtml(label)}</button>`).join("");
  }

  renderCrumbs() {
    const crumbs = [{ name: "All documents" }, ...this.path].map((crumb, depth) =>
      `<li class="sp-doc-browser__crumb" data-key="crumb-${depth}"><button type="button" class="sp-doc-browser__link" data-action="crumb"${attr("data-depth", depth)}>${escapeHtml(crumb.name)}</button></li>`);
    return `<ol class="sp-doc-browser__crumbs" aria-label="Folder path">${crumbs.join("")}</ol>`;
  }

  renderFolder(folder) {
    const ext = extensionsOf("folder", folder);
    return `<li class="sp-doc-browser__row sp-doc-browser__row--folder"${attr("data-key", `f-${folder.id}`)}${attr("data-folder-row", folder.id)}${ext.attrs}>
      <button type="button" class="sp-doc-browser__link" data-action="folder"${attr("data-id", folder.id)}${attr("data-name", folder.name)}>${escapeHtml(folder.name)}/</button>
      ${ext.slot}<span class="sp-doc-browser__meta">${escapeHtml(formatTime(folder.updated_at))}</span>${this.renderMenu(folder)}</li>`;
  }

  renderDocument(item) {
    const access = item.my_access ? `<span class="sp-doc-browser__badge">${escapeHtml(item.my_access)}</span>` : "";
    const ext = extensionsOf("document", item);
    return `<li class="sp-doc-browser__row"${attr("data-key", `d-${item.id}`)}${attr("data-document-row", item.id)}${ext.attrs}>
      <button type="button" class="sp-doc-browser__link" data-action="open"${attr("data-id", item.id)}${attr("data-format", item.format)}${attr("data-name", item.name)}>${escapeHtml(item.name)}</button>
      ${ext.slot}${access}<span class="sp-doc-browser__meta">${escapeHtml(formatBytes(item.size_bytes))}</span>
      <span class="sp-doc-browser__meta">${escapeHtml(formatTime(item.updated_at))}</span></li>`;
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
      <div class="sp-doc-browser__scopes" role="group" aria-label="Scope">${this.renderScopes()}</div>
      <form class="sp-doc-browser__search" role="search" data-submit="search">
        <input class="sp-doc-browser__filter" type="search" aria-label="Filter by name" data-input="filter"${attr("value", this.filter)}>
      </form>
      ${this.renderCrumbs()}${error}
      <ul class="sp-doc-browser__list">${folders.map((f) => this.renderFolder(f)).join("")}${documents.map((d) => this.renderDocument(d)).join("")}${empty}</ul>
    </section>`;
  }
}

customElements.define("sp-document-browser", SpDocumentBrowser);
