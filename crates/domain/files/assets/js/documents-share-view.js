import { attr, escapeHtml, formatTime } from "./documents-html.js";

/*
 * The read-side markup of the share dialog: who has access, what was
 * inherited from which folder, open access requests and the grant history.
 * Expired grants still render, greyed, until the sweep removes them.
 */
const subject = (grant) => `${escapeHtml(grant.subject_kind)}: ${escapeHtml(grant.display_name ?? grant.subject_value)}`;

const expiry = (grant) => {
  if (!grant.valid_until) { return ""; }
  const word = grant.expired ? "expired" : "until";
  return ` · ${word} ${escapeHtml(formatTime(grant.valid_until))}`;
};

export const renderGrant = (grant, { canRevoke }) => {
  const revoke = canRevoke
    ? `<button type="button" class="sp-share-dialog__button" data-action="revoke"${attr("data-id", grant.grant_id)}>Revoke</button>`
    : "";
  return `<li class="sp-share-dialog__grant"${attr("data-key", grant.grant_id)}${attr("data-expired", grant.expired ? "true" : null)}>
    <span class="sp-share-dialog__subject">${subject(grant)}</span>
    <span class="sp-share-dialog__level">${escapeHtml(grant.level)}${expiry(grant)}</span>${revoke}</li>`;
};

export const renderInherited = (grant) => {
  const from = grant.inherited_from?.name ?? grant.folder_id ?? "";
  return `<li class="sp-share-dialog__grant"${attr("data-key", `inherited-${grant.grant_id}`)}${attr("data-expired", grant.expired ? "true" : null)}>
    <span class="sp-share-dialog__subject">${subject(grant)}</span>
    <span class="sp-share-dialog__level">${escapeHtml(grant.level)} · from folder ${escapeHtml(from)}${expiry(grant)}</span></li>`;
};

export const renderWhoHasAccess = (access, { canRevoke }) => {
  const owner = `<li class="sp-share-dialog__grant" data-key="owner"><span class="sp-share-dialog__subject">owner: ${escapeHtml(access.owner)}</span><span class="sp-share-dialog__level">own</span></li>`;
  const direct = (access.grants ?? []).map((g) => renderGrant(g, { canRevoke })).join("");
  const inherited = (access.inherited ?? []).map(renderInherited).join("");
  return `<h3 class="sp-share-dialog__heading">Who has access</h3>
    <ul class="sp-share-dialog__grants">${owner}${direct}${inherited}</ul>
    <p class="sp-share-dialog__level">You hold ${escapeHtml(access.my_access)}.</p>`;
};

const renderRequest = (request) => {
  const message = request.message ? `<span class="sp-share-dialog__message">${escapeHtml(request.message)}</span>` : "";
  return `<li class="sp-share-dialog__grant"${attr("data-key", request.request_id)}>
    <span class="sp-share-dialog__subject">${escapeHtml(request.requester_user_id)} asks for ${escapeHtml(request.level)}</span>${message}
    <button type="button" class="sp-share-dialog__button" data-action="grant-request"${attr("data-id", request.request_id)}>Grant</button>
    <button type="button" class="sp-share-dialog__button" data-action="decline-request"${attr("data-id", request.request_id)}>Decline</button></li>`;
};

export const renderRequests = (requests) => {
  if (!requests?.length) { return ""; }
  return `<h3 class="sp-share-dialog__heading">Access requests</h3>
    <ul class="sp-share-dialog__grants">${requests.map(renderRequest).join("")}</ul>`;
};

const levels = (event) => [event.old_level, event.new_level].filter(Boolean).map(escapeHtml).join(" → ");

const renderEvent = (event, index) => {
  const actor = event.actor_user_id ? ` by ${escapeHtml(event.actor_user_id)}` : "";
  return `<li class="sp-share-dialog__grant"${attr("data-key", `${event.at}-${index}`)}>
    <span class="sp-share-dialog__subject">${escapeHtml(event.event)} ${escapeHtml(event.subject_kind)}: ${escapeHtml(event.subject_value)}</span>
    <span class="sp-share-dialog__level">${levels(event)}${actor} · ${escapeHtml(formatTime(event.at))}</span></li>`;
};

export const renderHistory = (history, { isOpen }) => {
  const toggle = `<button type="button" class="sp-share-dialog__button" data-action="toggle-history"${attr("aria-expanded", String(isOpen))}>Access history</button>`;
  if (!isOpen) { return toggle; }
  const rows = history?.length ? history.map(renderEvent).join("") : `<li class="sp-share-dialog__level">No changes recorded.</li>`;
  return `${toggle}<ul class="sp-share-dialog__grants" aria-label="Access history">${rows}</ul>`;
};
