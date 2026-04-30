import { apiPost } from "../api.js?t=__TOKEN__";
import { append, reportError } from "../drawer.js?t=__TOKEN__";
import { activateTab } from "../tabs.js?t=__TOKEN__";
import { selectMarketplaceKind, selectMarketplaceItem, setMarketplaceSearch, copyToClipboard } from "../marketplace.js?t=__TOKEN__";
import { connectFromSetup, editSetupPat, openSetupMode, closeSetupMode, completeSetup } from "../setup.js?t=__TOKEN__";

async function safePost(path, body, btn) {
  if (btn) {
    btn.dataset.busyLabel = btn.dataset.busyLabel || btn.textContent;
    btn.disabled = true;
    btn.setAttribute("aria-busy", "true");
  }
  try {
    await apiPost(path, body);
  } catch (e) {
    reportError(String(e.message || e));
  } finally {
    if (btn) {
      setTimeout(() => {
        btn.disabled = false;
        btn.removeAttribute("aria-busy");
      }, 600);
    }
  }
}

const ACTIONS = {
  sync: (btn) => safePost("/api/sync", undefined, btn),
  validate: (btn) => safePost("/api/validate", undefined, btn),
  "open-folder": (btn) => safePost("/api/open_folder", undefined, btn),
  recheck: (btn) => safePost("/api/probe", undefined, btn),
  logout: (btn) => safePost("/api/logout", undefined, btn),
  "change-gateway": () => openSetupMode(),
  "mkt-back": () => selectMarketplaceItem(null),
  "mkt-search-clear": () => setMarketplaceSearch(""),
  "setup-connect": () => connectFromSetup(),
  "setup-edit-pat": () => editSetupPat(),
  "setup-change-gateway": () => openSetupMode(),
  "setup-skip-agents": () => completeSetup(),
  "setup-finish": () => completeSetup(),
  "setup-open": () => closeSetupMode(),
};

function findAction(target) {
  return target.closest("[data-action]");
}

function dispatch(actionEl, event) {
  const action = actionEl.dataset.action;
  if (action === "tab") {
    const name = actionEl.dataset.tab;
    if (name) {
      activateTab(name);
    }
  } else if (action === "mkt-cat") {
    const kind = actionEl.dataset.kind;
    if (kind) {
      selectMarketplaceKind(kind);
    }
  } else if (action === "mkt-item") {
    const id = actionEl.dataset.id;
    if (id) {
      selectMarketplaceItem(id);
    }
  } else if (action === "mkt-install") {
    const id = actionEl.dataset.id;
    if (id) {
      safePost("/api/marketplace/install", { id }, actionEl);
    }
  } else if (action === "mkt-uninstall") {
    const id = actionEl.dataset.id;
    if (id) {
      safePost("/api/marketplace/uninstall", { id }, actionEl);
    }
  } else if (action === "mkt-copy") {
    const value = actionEl.dataset.value;
    if (value) {
      copyToClipboard(actionEl, value);
    }
  } else if (action === "host-generate") {
    const host = actionEl.dataset.host;
    if (host) {
      safePost(`/api/hosts/${encodeURIComponent(host)}/profile/generate`, undefined, actionEl);
    }
  } else if (action === "host-reverify") {
    const host = actionEl.dataset.host;
    if (host) {
      safePost(`/api/hosts/${encodeURIComponent(host)}/probe`, undefined, actionEl);
    }
  } else if (action === "host-install") {
    const host = actionEl.dataset.host;
    const path = actionEl.dataset.path;
    if (!host) {
      void 0;
    } else if (!path) {
      append(`[${host}] No generated profile yet — click Generate first.`);
    } else {
      safePost(`/api/hosts/${encodeURIComponent(host)}/profile/install`, { path }, actionEl);
    }
  } else if (action === "agent-jump") {
    const agent = actionEl.dataset.agent;
    if (agent) {
      activateTab("agents");
    }
  } else if (action === "setup-pat-link") {
    const aria = actionEl.getAttribute("aria-disabled");
    if (aria === "true") {
      event.preventDefault();
    }
  } else {
    const handler = ACTIONS[action];
    if (handler) {
      handler(actionEl);
    }
  }
}

export function initEvents() {
  document.addEventListener("click", (event) => {
    const actionEl = findAction(event.target);
    if (actionEl) {
      dispatch(actionEl, event);
    }
  });
}
