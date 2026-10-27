import { DocumentsElement } from "./documents-element.js";
import { getProjection, listRevisions, restoreRevision } from "./documents-api.js";
import { diffProjections } from "./documents-diff.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

export class SpHistory extends DocumentsElement {
  onConnect() {
    this.revisions = [];
    this.diff = null;
    this.on("action:restore", (el) => this.restore(el.dataset.id));
    this.on("action:compare", (el) => this.compare(el.dataset.id));
    this.onDocumentEvent((name) => { if (name === "document.revision.created") { this.load(); } });
    this.load();
  }

  async attempt(work) {
    try {
      await work();
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    this.invalidate();
  }

  load() {
    return this.attempt(async () => {
      ({ revisions: this.revisions } = await listRevisions(this.documentId, { limit: 50 }));
    });
  }

  restore(revisionId) {
    return this.attempt(() => restoreRevision(this.documentId, { revisionId, headRevision: this.revisions[0]?.id }));
  }

  compare(revisionId) {
    return this.attempt(async () => {
      const head = this.revisions[0]?.id;
      const [before, after] = await Promise.all([
        getProjection(this.documentId, { view: "outline", revision: revisionId }),
        getProjection(this.documentId, { view: "outline", revision: head }),
      ]);
      this.diff = { from: revisionId, lines: diffProjections(before.projection, after.projection) };
    });
  }

  renderRevision(revision, index) {
    const actions = index === 0 ? `<span class="sp-history__badge">head</span>` : `<button type="button" class="sp-history__button" data-action="compare"${attr("data-id", revision.id)}>Compare with head</button>
      <button type="button" class="sp-history__button" data-action="restore"${attr("data-id", revision.id)}>Restore</button>`;
    return `<li class="sp-history__revision"${attr("data-key", revision.id)}>
      <p class="sp-history__meta">${escapeHtml(formatTime(revision.created_at))} · ${escapeHtml(revision.author_user_id)} · ${escapeHtml(revision.origin_kind)}</p>
      <p class="sp-history__message">${escapeHtml(revision.message ?? "")}</p>${actions}</li>`;
  }

  renderDiff() {
    if (!this.diff) { return ""; }
    const lines = this.diff.lines.map((line) => `<li class="sp-history__line sp-history__line--${escapeHtml(line.change)}"${attr("data-key", `${line.change}-${line.id}`)}>
      <code class="sp-history__node">${escapeHtml(line.id)}</code> ${escapeHtml(line.change)}: ${escapeHtml(line.text)}${line.was == null ? "" : ` <del class="sp-history__was">${escapeHtml(line.was)}</del>`}</li>`).join("");
    return `<section class="sp-history__diff" aria-label="Changes since revision"><ol class="sp-history__lines">${lines || `<li class="sp-history__line">No changes</li>`}</ol></section>`;
  }

  render() {
    const error = this.error ? `<p class="sp-history__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    return `<section class="sp-history" aria-label="History">${error}
      <ol class="sp-history__revisions">${this.revisions.map((r, i) => this.renderRevision(r, i)).join("")}</ol>${this.renderDiff()}</section>`;
  }
}

customElements.define("sp-history", SpHistory);
