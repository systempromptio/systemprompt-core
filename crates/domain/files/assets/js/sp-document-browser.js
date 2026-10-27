import { DocumentsElement } from "./documents-element.js";
import { listDocuments } from "./documents-api.js";
import { attr, errorText, escapeHtml, formatBytes, formatTime } from "./documents-html.js";

const SCOPES = [["mine", "Mine"], ["shared", "Shared with me"], ["recent", "Recent"]];

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
    this.on("input:filter", (el) => { this.filter = el.value; this.invalidate(); });
    this.on("submit:search", () => this.load());
    this.onDocumentEvent((name) => { if (name === "document.revision.created") { this.load(); } });
    this.load();
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
    return `<li class="sp-doc-browser__row sp-doc-browser__row--folder"${attr("data-key", `f-${folder.id}`)}>
      <button type="button" class="sp-doc-browser__link" data-action="folder"${attr("data-id", folder.id)}${attr("data-name", folder.name)}>${escapeHtml(folder.name)}/</button>
      <span class="sp-doc-browser__meta">${escapeHtml(formatTime(folder.updated_at))}</span></li>`;
  }

  renderDocument(item) {
    const access = item.my_access ? `<span class="sp-doc-browser__badge">${escapeHtml(item.my_access)}</span>` : "";
    return `<li class="sp-doc-browser__row"${attr("data-key", `d-${item.id}`)}>
      <button type="button" class="sp-doc-browser__link" data-action="open"${attr("data-id", item.id)}${attr("data-format", item.format)}${attr("data-name", item.name)}>${escapeHtml(item.name)}</button>
      ${access}<span class="sp-doc-browser__meta">${escapeHtml(formatBytes(item.size_bytes))}</span>
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
