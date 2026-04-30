import { syncRailIndicator, initRailIndicator } from "./rail-indicator.js?t=__TOKEN__";
import { setCrumb } from "./crumb.js?t=__TOKEN__";

export const TAB_LABELS = {
  marketplace: "Marketplace",
  agents: "Agents",
  status: "Status",
  settings: "Settings",
};

export function activateTab(name) {
  for (const btn of document.querySelectorAll(".rail-tab[data-tab]")) {
    btn.setAttribute("aria-selected", btn.dataset.tab === name ? "true" : "false");
  }
  for (const panel of document.querySelectorAll(".tab-panel")) {
    panel.hidden = panel.dataset.tab !== name;
  }
  setCrumb(name);
  requestAnimationFrame(syncRailIndicator);
  try {
    localStorage.setItem("cowork.tab", name);
  } catch (_) {
    void 0;
  }
}

function readInitialTab() {
  try {
    return localStorage.getItem("cowork.tab") || "marketplace";
  } catch (_) {
    return "marketplace";
  }
}

export function initTabs() {
  initRailIndicator();
  activateTab(readInitialTab());
}
