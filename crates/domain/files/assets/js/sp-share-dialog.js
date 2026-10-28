import { DocumentsElement } from "./documents-element.js";
import {
  decideAccessRequest, getAccess, listSubjectKinds, revokeAllGrants, revokeGrant, shareDocument,
  transferOwnership,
} from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";
import { renderHistory, renderRequests, renderWhoHasAccess } from "./documents-share-view.js";

const LEVELS = ["view", "comment", "edit", "share"];
const RANK = { view: 1, comment: 2, edit: 3, share: 4, own: 5 };
const MAX_MESSAGE = 1024;

/*
 * Below `share` the dialog is read-only: the server returns only the owner
 * and the caller's level, so there is nothing else to show or act on.
 * Transfer and revoke-all are destructive and owner-only; each asks for a
 * confirmation in place before it runs.
 */
export class SpShareDialog extends DocumentsElement {
  onConnect() {
    this.access = null;
    this.subjectKinds = ["user"];
    this.isHistoryOpen = false;
    this.confirming = null;
    this.on("submit:share", (form) => this.share(form));
    this.on("submit:transfer", (form) => this.askToConfirm({ kind: "transfer", userId: form.elements.value.value }));
    this.on("action:revoke", (el) => this.run(() => revokeGrant(this.documentId, el.dataset.id)));
    this.on("action:revoke-all", () => this.askToConfirm({ kind: "revoke-all" }));
    this.on("action:confirm", () => this.runConfirmed());
    this.on("action:cancel", () => this.askToConfirm(null));
    this.on("action:toggle-history", () => this.toggleHistory());
    this.on("action:grant-request", (el) => this.run(() => decideAccessRequest(this.documentId, el.dataset.id, "grant")));
    this.on("action:decline-request", (el) => this.run(() => decideAccessRequest(this.documentId, el.dataset.id, "decline")));
    this.onDocumentEvent((name) => {
      if (name === "document.access_requested" || name === "document.shared") { this.load(); }
    });
    this.load();
  }

  get canShare() {
    return RANK[this.access?.my_access] >= RANK.share;
  }

  get isOwner() {
    return this.access?.my_access === "own";
  }

  async load() {
    try {
      const [access, kinds] = await Promise.all([
        getAccess(this.documentId, { history: this.isHistoryOpen }),
        listSubjectKinds(),
      ]);
      this.access = access;
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

  askToConfirm(pending) {
    this.confirming = pending;
    this.invalidate();
  }

  runConfirmed() {
    const pending = this.confirming;
    this.confirming = null;
    if (pending?.kind === "transfer") { return this.run(() => transferOwnership(this.documentId, pending.userId)); }
    if (pending?.kind === "revoke-all") { return this.run(() => revokeAllGrants(this.documentId)); }
    return this.invalidate();
  }

  toggleHistory() {
    this.isHistoryOpen = !this.isHistoryOpen;
    this.load();
  }

  share(form) {
    const { kind, value, level, expires, message } = form.elements;
    return this.run(() => shareDocument(this.documentId, {
      subjectKind: kind.value,
      subjectValue: value.value.trim(),
      level: level.value,
      validUntil: expires.value ? new Date(expires.value).toISOString() : null,
      message: message.value.trim(),
    }));
  }

  renderShareForm() {
    const levels = LEVELS.filter((l) => RANK[l] <= RANK[this.access.my_access])
      .map((l) => `<option${attr("value", l)}${attr("selected", l === "view")}>${escapeHtml(l)}</option>`).join("");
    return `<form class="sp-share-dialog__form" data-submit="share">
      <sp-subject-picker id="share-subject"${attr("kinds", this.subjectKinds.join(","))}></sp-subject-picker>
      <label class="sp-share-dialog__field">Level<select name="level">${levels}</select></label>
      <label class="sp-share-dialog__field">Expires<input name="expires" type="datetime-local"></label>
      <label class="sp-share-dialog__field">Message<textarea name="message"${attr("maxlength", MAX_MESSAGE)}></textarea></label>
      <button type="submit" class="sp-share-dialog__button">Share</button>
    </form>`;
  }

  renderOwnerActions() {
    if (!this.isOwner) { return ""; }
    const revokeAll = this.access.grants?.length
      ? `<button type="button" class="sp-share-dialog__button sp-share-dialog__button--danger" data-action="revoke-all">Revoke all</button>`
      : "";
    return `<form class="sp-share-dialog__form" data-submit="transfer">
      <sp-subject-picker id="transfer-subject" kinds="user" label="New owner"></sp-subject-picker>
      <button type="submit" class="sp-share-dialog__button">Transfer ownership</button>
    </form>${revokeAll}`;
  }

  renderConfirm() {
    if (!this.confirming) { return ""; }
    const question = this.confirming.kind === "transfer"
      ? "Transfer ownership? You keep edit access and lose the ability to undo this."
      : "Revoke every grant on this document?";
    return `<div class="sp-share-dialog__confirm" role="alertdialog" aria-label="Confirm">
      <p>${escapeHtml(question)}</p>
      <button type="button" class="sp-share-dialog__button sp-share-dialog__button--danger" data-action="confirm">Confirm</button>
      <button type="button" class="sp-share-dialog__button" data-action="cancel">Cancel</button></div>`;
  }

  render() {
    const error = this.error ? `<p class="sp-share-dialog__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    if (!this.access) { return `<section class="sp-share-dialog" aria-label="Share">${error}</section>`; }
    const view = renderWhoHasAccess(this.access, { canRevoke: this.canShare });
    if (!this.canShare) { return `<section class="sp-share-dialog" aria-label="Who has access">${view}${error}</section>`; }
    return `<section class="sp-share-dialog" aria-label="Share">
      ${this.renderShareForm()}${error}${this.renderConfirm()}${view}
      ${renderRequests(this.access.requests)}${this.renderOwnerActions()}
      ${renderHistory(this.access.history, { isOpen: this.isHistoryOpen })}
    </section>`;
  }
}

customElements.define("sp-share-dialog", SpShareDialog);
