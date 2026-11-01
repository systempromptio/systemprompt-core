import { escapeHtml, attr } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

const ATTENTION_LABEL = { human: "Needs you", agent: "For your agent", silent: "FYI" };
const ATTENTION_L10N = {
  human: "inbox-attention-human",
  agent: "inbox-attention-agent",
  silent: "inbox-attention-silent",
};

function label(id, fallback, args) {
  return t(id, args) || fallback;
}

function button(action, id, fallback) {
  return `<button type="button" class="sp-btn sp-btn--ghost sp-inbox-row__action" data-action="${action}">${escapeHtml(label(id, fallback))}</button>`;
}

function actions(row) {
  return [
    row.canOpen ? button("inbox-open", "inbox-open", "Open") : "",
    button("inbox-copy", "inbox-copy-prompt", "Copy prompt"),
    row.isUnread ? button("inbox-read", "inbox-mark-read", "Mark read") : "",
    row.canEscalate ? button("inbox-escalate", "inbox-escalate", "Escalate") : "",
    row.canStartSession ? button("inbox-start", "inbox-start-session", "Start session") : "",
  ].join("");
}

function escalateForm() {
  const noteLabel = label("inbox-escalate-note", "Note for the reader (optional)");
  return `
    <div class="sp-inbox-row__escalate">
      <label class="sp-inbox-row__note-label">${escapeHtml(noteLabel)}
        <input type="text" class="sp-inbox-row__note" data-note ${attr("aria-label", noteLabel)}>
      </label>
      ${button("inbox-escalate-send", "inbox-escalate-send", "Escalate to me")}
      ${button("inbox-escalate-cancel", "inbox-cancel", "Cancel")}
    </div>`;
}

function copyFallback(prompt) {
  const hint = label("inbox-copy-fallback", "Copy the prompt below");
  return `<textarea class="sp-inbox-row__prompt" readonly data-prompt ${attr("aria-label", hint)}>${escapeHtml(prompt)}</textarea>`;
}

export function renderInboxRow(row, { escalating, fallbackPrompt }) {
  const unread = row.isUnread ? " sp-inbox-row--unread" : "";
  const count = row.count == null
    ? ""
    : `<span class="sp-badge sp-inbox-row__count">${escapeHtml(label("inbox-coalesced", `×${row.count}`, { count: row.count }))}</span>`;
  const via = row.viaAgent
    ? `<span class="sp-inbox-row__via">${escapeHtml(label("inbox-via", `via ${row.viaAgent}`, { agent: row.viaAgent }))}</span>`
    : "";
  return `
    <li class="sp-inbox-row${unread}" data-key="${escapeHtml(row.key)}">
      <div class="sp-inbox-row__head">
        <span class="sp-chip sp-inbox-row__chip sp-inbox-row__chip--${escapeHtml(row.attention)}">${escapeHtml(label(ATTENTION_L10N[row.attention], ATTENTION_LABEL[row.attention]))}</span>
        <span class="sp-inbox-row__title">${escapeHtml(row.title)}</span>
        ${via}
        ${count}
        <time class="sp-inbox-row__time" ${attr("datetime", row.createdAt)}>${escapeHtml(row.age)}</time>
      </div>
      <p class="sp-inbox-row__text">${escapeHtml(row.text)}</p>
      <div class="sp-inbox-row__actions">${actions(row)}</div>
      ${escalating ? escalateForm() : ""}
      ${fallbackPrompt == null ? "" : copyFallback(fallbackPrompt)}
    </li>`;
}
