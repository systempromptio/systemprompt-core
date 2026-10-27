import { DocumentsElement } from "./documents-element.js";
import { listGrants, revokeGrant, shareDocument } from "./documents-api.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

const LEVELS = ["view", "comment", "edit", "share", "own"];

export class SpShareDialog extends DocumentsElement {
  onConnect() {
    this.grants = [];
    this.on("submit:share", (form) => this.share(form, form.elements.level.value));
    this.on("action:transfer", (el) => this.share(el.closest("form"), "own"));
    this.on("action:revoke", (el) => this.revoke([el.dataset.id]));
    this.on("action:revoke-all", () => this.revoke(this.grants.map((g) => g.id)));
    this.load();
  }

  async load() {
    try {
      ({ grants: this.grants } = await listGrants(this.documentId));
      this.error = null;
    } catch (err) {
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

  share(form, level) {
    const { kind, value, expires } = form.elements;
    return this.run(() => shareDocument(this.documentId, {
      subjectKind: kind.value.trim(),
      subjectValue: value.value.trim(),
      level,
      validUntil: expires.value ? new Date(expires.value).toISOString() : null,
    }));
  }

  revoke(ids) {
    return this.run(async () => {
      for (const id of ids) { await revokeGrant(this.documentId, id); }
    });
  }

  kinds() {
    return [...new Set(["user", ...this.grants.map((g) => g.subject_kind)])];
  }

  renderGrant(grant) {
    const expiry = grant.valid_until ? ` · until ${escapeHtml(formatTime(grant.valid_until))}` : "";
    return `<li class="sp-share-dialog__grant"${attr("data-key", grant.id)}>
      <span class="sp-share-dialog__subject">${escapeHtml(grant.subject_kind)}: ${escapeHtml(grant.subject_value)}</span>
      <span class="sp-share-dialog__level">${escapeHtml(grant.access_level)}${expiry}</span>
      <button type="button" class="sp-share-dialog__button" data-action="revoke"${attr("data-id", grant.id)}>Revoke</button></li>`;
  }

  renderForm() {
    const kinds = this.kinds().map((k) => `<option${attr("value", k)}></option>`).join("");
    const levels = LEVELS.map((l) => `<option${attr("value", l)}${attr("selected", l === "view")}>${escapeHtml(l)}</option>`).join("");
    return `<form class="sp-share-dialog__form" data-submit="share">
      <label class="sp-share-dialog__field">Subject kind<input name="kind" list="sp-share-kinds" required value="user"></label>
      <datalist id="sp-share-kinds">${kinds}</datalist>
      <label class="sp-share-dialog__field">Subject<input name="value" required></label>
      <label class="sp-share-dialog__field">Level<select name="level">${levels}</select></label>
      <label class="sp-share-dialog__field">Expires<input name="expires" type="datetime-local"></label>
      <button type="submit" class="sp-share-dialog__button">Share</button>
      <button type="button" class="sp-share-dialog__button" data-action="transfer">Transfer ownership</button>
    </form>`;
  }

  render() {
    const error = this.error ? `<p class="sp-share-dialog__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    const revokeAll = this.grants.length ? `<button type="button" class="sp-share-dialog__button sp-share-dialog__button--danger" data-action="revoke-all">Revoke all</button>` : "";
    return `<section class="sp-share-dialog" aria-label="Share">
      ${this.renderForm()}${error}
      <ul class="sp-share-dialog__grants">${this.grants.map((g) => this.renderGrant(g)).join("")}</ul>${revokeAll}
    </section>`;
  }
}

customElements.define("sp-share-dialog", SpShareDialog);
