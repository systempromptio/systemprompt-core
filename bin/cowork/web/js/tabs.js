import { $ } from "./dom.js?t=__TOKEN__";

const TAB_LABELS = {
  marketplace: "Marketplace",
  agents: "Agents",
  status: "Status",
  settings: "Settings",
};

export function syncRailIndicator() {
  const rail = document.querySelector(".rail");
  if (!rail) return;
  const active = rail.querySelector('.rail-tab[data-tab][aria-selected="true"]');
  if (!active) { rail.dataset.activeReady = "false"; return; }
  const railRect = rail.getBoundingClientRect();
  const tabRect = active.getBoundingClientRect();
  const y = (tabRect.top - railRect.top) + rail.scrollTop;
  rail.style.setProperty("--sp-rail-active-y", `${y}px`);
  rail.style.setProperty("--sp-rail-active-h", `${tabRect.height}px`);
  rail.dataset.activeReady = "true";
}

function setCrumb(name) {
  const crumb = $("crumb-current");
  if (!crumb) return;
  const label = TAB_LABELS[name] || name || "";
  if (crumb.textContent === label) return;
  const nav = $("crumb");
  if (nav) nav.dataset.changing = "true";
  setTimeout(() => {
    crumb.textContent = label;
    if (nav) nav.dataset.changing = "false";
  }, 120);
}

export function activateTab(name) {
  for (const btn of document.querySelectorAll(".rail-tab[data-tab]")) {
    btn.setAttribute("aria-selected", btn.dataset.tab === name ? "true" : "false");
  }
  for (const panel of document.querySelectorAll(".tab-panel")) {
    panel.hidden = panel.dataset.tab !== name;
  }
  setCrumb(name);
  requestAnimationFrame(syncRailIndicator);
  try { localStorage.setItem("cowork.tab", name); } catch (_) {}
}

export function initTabs() {
  for (const btn of document.querySelectorAll(".rail-tab[data-tab]")) {
    btn.addEventListener("click", () => activateTab(btn.dataset.tab));
  }
  const initial = (() => {
    try { return localStorage.getItem("cowork.tab") || "marketplace"; } catch (_) { return "marketplace"; }
  })();
  activateTab(initial);
  window.addEventListener("resize", syncRailIndicator);

  document.addEventListener("keydown", (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    const t = e.target;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
    if (e.key === "1") { e.preventDefault(); activateTab("marketplace"); }
    else if (e.key === "2") { e.preventDefault(); activateTab("agents"); }
    else if (e.key === "3") { e.preventDefault(); activateTab("status"); }
    else if (e.key === "4") { e.preventDefault(); activateTab("settings"); }
  });

  document.addEventListener("click", (e) => {
    const target = e.target.closest("[data-jump-tab]");
    if (!target) return;
    e.preventDefault();
    activateTab(target.dataset.jumpTab);
  });

  window.addEventListener("sp-jump-tab", (e) => {
    if (e.detail && e.detail.tab) activateTab(e.detail.tab);
  });
}
