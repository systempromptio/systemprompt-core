import { DocumentsElement } from "./documents-element.js";
import { listComments, postComment, resolveComment } from "./documents-api.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

export class SpComments extends DocumentsElement {
  onConnect() {
    this.comments = [];
    this.on("submit:comment", (form) => this.post(form));
    this.on("action:resolve", (el) => this.run(() => resolveComment(this.documentId, el.dataset.id)));
    this.onDocumentEvent((name) => { if (name.startsWith("document.comment")) { this.load(); } });
    this.load();
  }

  async load() {
    try {
      ({ comments: this.comments } = await listComments(this.documentId));
      this.error = null;
    } catch (err) {
      if (err.status === 404) {
        this.hidden = true;
        return;
      }
      this.error = err;
    }
    this.invalidate();
  }

  async run(work) {
    try {
      await work();
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    await this.load();
  }

  post(form) {
    const { body, node, parent } = form.elements;
    const anchor = node?.value.trim() ? { node: node.value.trim() } : undefined;
    return this.run(async () => {
      await postComment(this.documentId, { anchor, body: body.value, parentId: parent?.value || undefined });
      form.reset();
    });
  }

  renderComment(comment) {
    return `<li class="sp-comments__comment"${attr("data-key", comment.id)}>
      <p class="sp-comments__meta">${escapeHtml(comment.author ?? comment.author_user_id ?? "")} · ${escapeHtml(formatTime(comment.created_at))}</p>
      <p class="sp-comments__body">${escapeHtml(comment.body)}</p></li>`;
  }

  renderThread(root) {
    const replies = this.comments.filter((c) => c.parent_id === root.id);
    const anchor = root.anchor?.node ? `<p class="sp-comments__anchor">on ${escapeHtml(root.anchor.node)}</p>` : "";
    const resolve = root.resolved_at ? `<p class="sp-comments__meta">Resolved</p>` : `<button type="button" class="sp-comments__button" data-action="resolve"${attr("data-id", root.id)}>Resolve</button>`;
    return `<li class="sp-comments__thread${root.resolved_at ? " sp-comments__thread--resolved" : ""}"${attr("data-key", root.id)}>${anchor}
      <ol class="sp-comments__messages">${[root, ...replies].map((c) => this.renderComment(c)).join("")}</ol>
      <form class="sp-comments__form" data-submit="comment"><input type="hidden" name="parent"${attr("value", root.id)}>
        <input class="sp-comments__input" name="body" required aria-label="Reply"><button type="submit" class="sp-comments__button">Reply</button></form>
      ${resolve}</li>`;
  }

  render() {
    const error = this.error ? `<p class="sp-comments__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    const threads = this.comments.filter((c) => !c.parent_id).map((c) => this.renderThread(c)).join("");
    return `<section class="sp-comments" aria-label="Comments">${error}
      <ol class="sp-comments__threads">${threads}</ol>
      <form class="sp-comments__form" data-submit="comment">
        <input class="sp-comments__input" name="node" aria-label="Anchor node id"${attr("value", this.getAttribute("anchor-node") ?? "")}>
        <input class="sp-comments__input" name="body" required aria-label="New comment"><button type="submit" class="sp-comments__button">Comment</button></form>
    </section>`;
  }
}

customElements.define("sp-comments", SpComments);
