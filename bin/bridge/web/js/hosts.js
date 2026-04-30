import { $ } from "./dom.js?t=__TOKEN__";
import { refsFromNode, renderHostCard } from "./hosts/card.js?t=__TOKEN__";

const hostCards = new Map();

const wireButtons = (refs, id) => {
  if (refs.btnGenerate) {
    refs.btnGenerate.dataset.action = "host-generate";
    refs.btnGenerate.dataset.host = id;
  }
  if (refs.btnReverify) {
    refs.btnReverify.dataset.action = "host-reverify";
    refs.btnReverify.dataset.host = id;
  }
  if (refs.btnInstall) {
    refs.btnInstall.dataset.action = "host-install";
    refs.btnInstall.dataset.host = id;
  }
};

export const getOrCreateHostCard = (id) => {
  const existing = hostCards.get(id);
  if (existing) {
    return existing;
  }
  const tmpl = $("host-card-template");
  if (!tmpl) {
    return null;
  }
  const node = tmpl.content.firstElementChild.cloneNode(true);
  node.dataset.hostId = id;
  const refs = refsFromNode(node);
  wireButtons(refs, id);
  $("hosts-list").append(node);
  hostCards.set(id, refs);
  return refs;
};

const removeStaleCards = (presentIds) => {
  for (const [id, refs] of hostCards.entries()) {
    if (!presentIds.has(id)) {
      refs.root.remove();
      hostCards.delete(id);
    }
  }
};

const renderEmptyHosts = (list) => {
  if (list && list.children.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted host-list-empty";
    empty.textContent = "No host apps registered on this platform.";
    list.replaceChildren(empty);
  }
};

const clearEmptyMessage = (placeholder) => {
  const noHostsMsg = placeholder && placeholder.querySelector(":scope > .host-list-empty");
  if (noHostsMsg) {
    noHostsMsg.remove();
  }
};

export { renderHostCard } from "./hosts/card.js?t=__TOKEN__";

export const renderHosts = (snap) => {
  const list = snap.host_apps || [];
  const presentIds = new Set(list.map((h) => h.id));
  removeStaleCards(presentIds);
  const placeholder = $("hosts-list");
  if (list.length === 0) {
    renderEmptyHosts(placeholder);
    return;
  }
  clearEmptyMessage(placeholder);
  for (const host of list) {
    const refs = getOrCreateHostCard(host.id);
    if (refs) {
      renderHostCard(refs, host, snap);
    }
  }
};
