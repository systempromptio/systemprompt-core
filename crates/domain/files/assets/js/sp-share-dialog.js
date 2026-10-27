import { DocumentsElement } from "./documents-element.js";
import {
  getAccess, listSubjectKinds, revokeAllGrants, revokeGrant, shareDocument, transferOwnership,
} from "./documents-api.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

const LEVELS = ["view", "comment", "edit", "share", "own"];

export class SpShareDialog extends DocumentsElement {
  onConnect() {
    this.grants = [];
    this.subjectKinds = ["user"];
    this.on("submit:share", (form) => this.share(form));
    this.on("action:transfer", (el) => this.transfer(el.closest("form")));
    this.on("action:revoke", (el) => this.run(() => revokeGrant(this.documentId, el.dataset.id)));
    this.on("action:revoke-all", () => this.run(() => revokeAllGrants(this.documentId)));
    this.load();
  }

  async load() {
    try {
      const [access, kinds] = await Promise.all([getAccess(this.documentId), listSubjectKinds()]);
      this.grants = access.grants;
      this.subjectKinds = kinds.subject_kinds;
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

  share(form) {
    const { kind, value, level, expires } = form.elements;
    return this.run(() => shareDocument(this.documentId, {
      subjectKind: kind.value,
      subjectValue: value.value.trim(),
      level: level.value,
      validUntil: expires.value ? new Date(expires.value).toISOString() : null,
    }));
  }

  transfer(form) {
    return this.run(() => transferOwnership(this.documentId, form.elements.value.value.trim()));
  }

  renderGrant(grant) {
    const expiry = grant.valid_until ? ` · until ${escapeHtml(formatTime(grant.valid_until))}` : "";
    return `<li class="sp-share-dialog__grant"${attr("data-key", grant.grant_id)}>
      <span class="sp-share-dialog__subject">${escapeHtml(grant.subject_kind)}: ${escapeHtml(grant.display_name ?? grant.subject_value)}</span>
      <span class="sp-share-dialog__level">${escapeHtml(grant.level)}${expiry}</span>
      <button type="button" class="sp-share-dialog__button" data-action="revoke"${attr("data-id", grant.grant_id)}>Revoke</button></li>`;
  }

  renderForm() {
    const kinds = this.subjectKinds.map((k) => `<option${attr("value", k)}${attr("selected", k === "user")}>${escapeHtml(k)}</option>`).join("");
    const levels = LEVELS.map((l) => `<option${attr("value", l)}${attr("selected", l === "view")}>${escapeHtml(l)}</option>`).join("");
    return `<form class="sp-share-dialog__form" data-submit="share">
      <label class="sp-share-dialog__field">Subject kind<select name="kind">${kinds}</select></label>
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
