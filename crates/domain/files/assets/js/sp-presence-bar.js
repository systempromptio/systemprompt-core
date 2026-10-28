import { DocumentsElement } from "./documents-element.js";
import { getPresence } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";

const initials = (userId) => String(userId).slice(0, 2).toUpperCase();

export class SpPresenceBar extends DocumentsElement {
  onConnect() {
    this.editors = new Map();
    this.onDocumentEvent((name, value) => {
      if (name !== "document.presence") { return; }
      this.editors.set(value.user_id, { user_id: value.user_id, state: value.state, source: value.source_kind });
      this.invalidate();
    });
    this.load();
  }

  async load() {
    try {
      const { editors } = await getPresence(this.documentId);
      for (const editor of editors ?? []) { this.editors.set(editor.user_id, editor); }
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    this.invalidate();
  }

  render() {
    const people = [...this.editors.values()].map((e) => `<li class="sp-presence-bar__person"${attr("data-key", e.user_id)}${attr("title", `${e.user_id} (${e.state ?? "viewing"}, ${e.source ?? "web"})`)}>
      <span class="sp-presence-bar__avatar" aria-hidden="true">${escapeHtml(initials(e.user_id))}</span>
      <span class="sp-u-sr-only">${escapeHtml(e.user_id)} ${escapeHtml(e.state ?? "")}</span></li>`);
    if (this.error) { return `<p class="sp-presence-bar__error">${escapeHtml(errorText(this.error))}</p>`; }
    return `<ul class="sp-presence-bar" aria-label="People here" aria-live="polite">${people.join("")}</ul>`;
  }
}

customElements.define("sp-presence-bar", SpPresenceBar);
