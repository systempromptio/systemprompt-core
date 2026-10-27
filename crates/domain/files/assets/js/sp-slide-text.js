import { DocumentsElement } from "./documents-element.js";
import { getProjection, opId, postEdits } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";

const leaves = (node) => (node.children?.length ? node.children.flatMap(leaves) : [node]);

const isText = (node) => typeof node.text === "string" && ["paragraph", "heading", "list_item", "text"].includes(node.kind?.type);

export class SpSlideText extends DocumentsElement {
  onConnect() {
    this.on("change:text", (el) => this.save(el));
    this.onDocumentEvent((name) => { if (name === "document.revision.created") { this.load(); } });
    this.load();
  }

  async load() {
    try {
      const { revision_id: revision, projection } = await getProjection(this.documentId, { view: "outline" });
      this.revision = revision;
      this.slides = projection.root.filter((node) => node.kind?.type === "slide");
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    this.invalidate();
  }

  async save(input) {
    const { node, hash } = input.dataset;
    const op = { op: "replace_text", node, expected_hash: hash, text: input.value };
    try {
      await postEdits(this.documentId, { baseRevision: this.revision, ops: [op], message: "edit slide text", opId: opId() });
      this.error = null;
    } catch (err) {
      this.error = err;
      this.invalidate();
    }
  }

  renderParagraph(node) {
    return `<li class="sp-slide-text__para"${attr("data-key", node.id)}>
      <input class="sp-slide-text__input" data-change="text"${attr("data-node", node.id)}${attr("data-hash", node.hash8)}${attr("value", node.text)}${attr("aria-label", `Text ${node.id}`)}></li>`;
  }

  renderSlide(slide, index) {
    const paragraphs = leaves(slide).filter(isText).map((node) => this.renderParagraph(node)).join("");
    return `<li class="sp-slide-text__slide"${attr("data-key", slide.id)}>
      <h3 class="sp-slide-text__title">${index + 1}. ${escapeHtml(slide.text ?? "")}</h3>
      <ol class="sp-slide-text__paras">${paragraphs}</ol></li>`;
  }

  render() {
    const error = this.error ? `<p class="sp-slide-text__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    const slides = (this.slides ?? []).map((slide, i) => this.renderSlide(slide, i)).join("");
    return `<section class="sp-slide-text">${error}<ol class="sp-slide-text__slides">${slides}</ol></section>`;
  }
}

customElements.define("sp-slide-text", SpSlideText);
