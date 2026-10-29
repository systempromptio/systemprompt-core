import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { EMPTY_INBOX, inboxRows, unreadCount, unreadLabel } from "/assets/js/utils/inbox-rows.js";
import { renderInboxRow } from "/assets/js/components/inbox-row.js";
import { registerInboxActions } from "/assets/js/components/inbox-actions.js";
import { watchSeen } from "/assets/js/components/inbox-seen.js";

export class SpInbox extends SpElement {
  constructor() {
    super();
    this.inbox = EMPTY_INBOX;
    this.escalating = null;
    this.fallback = null;
    registerInboxActions(this);
  }

  onConnect() {
    this.classList.add("sp-inbox");
    this.bridgeSubscribe("inbox.changed", (payload) => this._accept(payload));
    const seen = watchSeen(this);
    this._unsubs.push(seen.stop);
    this._reportSeen = seen.report;
    this._load();
  }

  async _load() {
    try {
      this._accept(await bridge.invoke("inbox.list", {}, { timeoutMs: 30_000 }));
    } catch (err) {
      console.error("inbox.list failed", err);
      this._accept(EMPTY_INBOX);
    }
  }

  _accept(payload) {
    this.inbox = payload || EMPTY_INBOX;
    const detail = { unread: unreadCount(this.inbox), label: unreadLabel(this.inbox) };
    document.dispatchEvent(new CustomEvent("inbox:count", { detail }));
  }

  afterRender() {
    this._reportSeen?.();
    if (!this.fallback) { return; }
    const field = this.querySelector("[data-prompt]");
    if (field && document.activeElement !== field) { field.focus(); field.select(); }
  }

  _renderHeader(unread) {
    const heading = t("nav-inbox") || "Inbox";
    const markAll = unread > 0
      ? `<button type="button" class="sp-btn sp-btn--ghost" data-action="inbox-read-all">${escapeHtml(t("inbox-mark-all-read") || "Mark all read")}</button>`
      : "";
    return `
      <header class="sp-tab__header sp-inbox__header">
        <h1 data-l10n-id="nav-inbox">${escapeHtml(heading)}</h1>
        ${markAll}
      </header>`;
  }

  render() {
    const inbox = this.inbox || EMPTY_INBOX;
    if (inbox.available === false) {
      const note = t("inbox-unavailable") || "Notifications are not enabled on this server.";
      return `${this._renderHeader(0)}<p class="sp-inbox__empty" data-l10n-id="inbox-unavailable">${escapeHtml(note)}</p>`;
    }
    const rows = inboxRows(inbox);
    const error = inbox.last_error
      ? `<p class="sp-inbox__error" role="status">${escapeHtml(inbox.last_error)}</p>`
      : "";
    const body = rows.length === 0
      ? `<p class="sp-inbox__empty">${escapeHtml(t("inbox-empty") || "Nothing needs you right now.")}</p>`
      : `<ul class="sp-inbox__list" aria-live="polite">${rows.map((row) => renderInboxRow(row, {
        escalating: this.escalating === row.key,
        fallbackPrompt: this.fallback?.id === row.key ? this.fallback.prompt : null,
      })).join("")}</ul>`;
    const older = inbox.has_older
      ? `<button type="button" class="sp-btn sp-btn--ghost" data-action="inbox-load-older" data-l10n-id="inbox-load-older">${escapeHtml(t("inbox-load-older") || "Load older")}</button>`
      : "";
    return `${this._renderHeader(unreadCount(inbox))}${error}${body}${older}`;
  }
}

reactive(SpInbox.prototype, ["inbox", "escalating", "fallback"]);
customElements.define("sp-inbox", SpInbox);
