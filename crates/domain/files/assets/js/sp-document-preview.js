import { DocumentsElement } from "./documents-element.js";
import { previewUrl, touchPresence } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";

/*
 * The preview HTML is sanitised server-side and served SAMEORIGIN; the frame
 * is still sandboxed without scripts so a sanitiser miss cannot run in the
 * page's origin.
 */
export class SpDocumentPreview extends DocumentsElement {
  static get observedAttributes() { return ["document-id", "revision"]; }

  attributeChangedCallback() {
    this.invalidate();
  }

  onConnect() {
    this.onDocumentEvent((name, value) => {
      if (name === "document.revision.created" && !this.hasAttribute("revision")) {
        this.headRevision = value.revision_id;
        this.invalidate();
      }
    });
    if (this.documentId) {
      this.announce();
    }
  }

  async announce() {
    try {
      await touchPresence(this.documentId, "viewing");
    } catch (err) {
      this.error = err;
      this.invalidate();
    }
  }

  render() {
    if (!this.documentId) { return `<p class="sp-doc-preview__empty">Select a document</p>`; }
    const revision = this.getAttribute("revision") ?? undefined;
    const src = `${previewUrl(this.documentId, revision)}${this.headRevision ? `${revision ? "&" : "?"}head=${encodeURIComponent(this.headRevision)}` : ""}`;
    return `<section class="sp-doc-preview">
      <header class="sp-doc-preview__bar"><sp-presence-bar${attr("document-id", this.documentId)}></sp-presence-bar></header>
      ${this.error ? `<p class="sp-doc-preview__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : ""}
      <iframe class="sp-doc-preview__frame" sandbox="" referrerpolicy="no-referrer"${attr("title", `Preview ${this.getAttribute("name") ?? ""}`)}${attr("src", src)}></iframe>
    </section>`;
  }
}

customElements.define("sp-document-preview", SpDocumentPreview);
