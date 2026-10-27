import { DocumentsElement } from "./documents-element.js";
import { decideProposal, listProposals } from "./documents-api.js";
import { attr, errorText, escapeHtml, formatTime } from "./documents-html.js";

export class SpProposalList extends DocumentsElement {
  onConnect() {
    this.proposals = [];
    this.on("action:decide", (el) => this.decide(el));
    this.onDocumentEvent((name) => { if (name.startsWith("document.proposal") || name === "document.revision.created") { this.load(); } });
    this.load();
  }

  async load() {
    try {
      ({ proposals: this.proposals } = await listProposals(this.documentId));
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

  async decide(el) {
    const { id, decision } = el.dataset;
    const note = this.querySelector(`[data-note="${CSS.escape(id)}"]`)?.value ?? "";
    try {
      await decideProposal(this.documentId, id, decision, note || undefined);
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    await this.load();
  }

  renderActions(proposal) {
    if (proposal.status !== "open" && proposal.status !== "pending") { return ""; }
    return `<input class="sp-proposal-list__note" aria-label="Note"${attr("data-note", proposal.id)}>
      <button type="button" class="sp-proposal-list__button" data-action="decide" data-decision="accept"${attr("data-id", proposal.id)}>Accept</button>
      <button type="button" class="sp-proposal-list__button" data-action="decide" data-decision="reject"${attr("data-id", proposal.id)}>Reject</button>`;
  }

  renderProposal(proposal) {
    return `<li class="sp-proposal-list__item sp-proposal-list__item--${escapeHtml(proposal.status)}"${attr("data-key", proposal.id)}>
      <p class="sp-proposal-list__message">${escapeHtml(proposal.message ?? "(no message)")}</p>
      <p class="sp-proposal-list__meta">${escapeHtml(proposal.proposer ?? "")} · ${escapeHtml(formatTime(proposal.created_at))} · ${escapeHtml(proposal.status)}</p>
      ${this.renderActions(proposal)}</li>`;
  }

  render() {
    const error = this.error ? `<p class="sp-proposal-list__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    const items = this.proposals.map((p) => this.renderProposal(p)).join("") || `<li class="sp-proposal-list__empty">No proposals</li>`;
    return `<section class="sp-proposal-list" aria-label="Proposals">${error}<ul class="sp-proposal-list__items">${items}</ul></section>`;
  }
}

customElements.define("sp-proposal-list", SpProposalList);
