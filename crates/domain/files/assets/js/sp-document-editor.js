import { DocumentsElement } from "./documents-element.js";
import { CollabSession } from "./documents-collab.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

const CURSOR_DELAY_MS = 400;

export class SpDocumentEditor extends DocumentsElement {
  onConnect() {
    this.session = new CollabSession(this.documentId);
    this.status = "joining";
    this.checkpoint = null;
    this.cursors = new Map();
    this.on("input:body", (el) => this.send(el.value));
    this.onDocumentEvent((name, value) => this.handleEvent(name, value));
    const unload = () => this.session.leave({ unloading: true });
    window.addEventListener("pagehide", unload);
    this.own(() => window.removeEventListener("pagehide", unload));
    const select = () => this.scheduleCursor();
    document.addEventListener("selectionchange", select);
    this.own(() => document.removeEventListener("selectionchange", select));
    this.start();
  }

  onDisconnect() {
    clearTimeout(this.cursorTimer);
    this.session.leave()?.catch((err) => { this.error = err; });
  }

  get area() {
    return this.querySelector("[data-input='body']");
  }

  async start() {
    try {
      await this.session.join(this.hasAttribute("readonly") ? "view" : "edit");
      this.status = "live";
      this.invalidate();
      queueMicrotask(() => { this.area.value = this.session.text(); });
    } catch (err) {
      this.fail(err);
    }
  }

  fail(err) {
    this.error = err;
    this.status = "error";
    this.invalidate();
  }

  async send(value) {
    try {
      await this.session.push(value);
      this.status = "live";
    } catch (err) {
      this.fail(err);
    }
  }

  handleEvent(name, value) {
    if (name === "document.collab.update" && this.session.importRemote(value)) { this.syncArea(); }
    if (name === "document.revision.created" && value.origin_kind === "edit_session") {
      this.checkpoint = { revision: value.revision_id, at: new Date().toISOString() };
      this.invalidate();
    }
    if (name === "document.presence" && value.cursor) {
      this.cursors.set(value.user_id, value.cursor);
      this.invalidate();
    }
  }

  syncArea() {
    const { area } = this;
    const next = this.session.text();
    if (!area || area.value === next) { return; }
    const { selectionStart, selectionEnd } = area;
    const shift = next.length - area.value.length;
    const before = area.value.slice(0, selectionStart);
    const kept = next.startsWith(before);
    area.value = next;
    area.setSelectionRange(kept ? selectionStart : selectionStart + shift, kept ? selectionEnd : selectionEnd + shift);
  }

  scheduleCursor() {
    if (document.activeElement !== this.area || this.status !== "live") { return; }
    clearTimeout(this.cursorTimer);
    this.cursorTimer = setTimeout(() => this.sendCursor(), CURSOR_DELAY_MS);
  }

  async sendCursor() {
    const { selectionStart, selectionEnd } = this.area;
    try {
      await this.session.cursor({ anchor: selectionStart, head: selectionEnd });
    } catch (err) {
      this.fail(err);
    }
  }

  renderStatus() {
    const saved = this.checkpoint ? ` · checkpoint ${escapeHtml(formatTime(this.checkpoint.at))}` : "";
    const error = this.error ? `<span class="sp-doc-editor__error" role="alert">${escapeHtml(errorText(this.error))}</span>` : "";
    const cursors = [...this.cursors].map(([user, c]) => `<li class="sp-doc-editor__cursor"${attr("data-key", user)}>${escapeHtml(user)} @ ${escapeHtml(c.anchor ?? "")}</li>`).join("");
    return `<footer class="sp-doc-editor__status" aria-live="polite">${escapeHtml(this.status)}${saved}${error}
      <ul class="sp-doc-editor__cursors">${cursors}</ul></footer>`;
  }

  render() {
    return `<section class="sp-doc-editor${this.status === "error" ? " sp-doc-editor--error" : ""}">
      <header class="sp-doc-editor__bar"><sp-presence-bar${attr("document-id", this.documentId)}></sp-presence-bar></header>
      <textarea class="sp-doc-editor__body" data-input="body" data-preserve spellcheck="false" aria-label="Document source"${attr("readonly", this.hasAttribute("readonly") || this.status !== "live")}></textarea>
      ${this.renderStatus()}
    </section>`;
  }
}

customElements.define("sp-document-editor", SpDocumentEditor);
