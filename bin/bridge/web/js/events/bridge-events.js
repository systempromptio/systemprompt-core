import { TAB_KEYS, isTextInput } from "/assets/js/utils/rail-tabs.js";

const FORWARDED_EVENTS = ["mkt:count", "crumb:set", "setup-open"];
const handlers = new Map();
for (const name of FORWARDED_EVENTS) { handlers.set(name, new Set()); }

function focusMarketplaceSearch() {
  const search = document.getElementById("mkt-search");
  if (search) { search.focus(); search.select(); return true; }
  return false;
}

function activateRailTab(name) {
  const rail = document.querySelector("sp-rail");
  if (rail && typeof rail.activateTab === "function") {
    rail.activateTab(name);
  }
}

function handleKeydown(e) {
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) { return; }
  if (e.key === "f") {
    if (focusMarketplaceSearch()) { e.preventDefault(); }
    return;
  }
  if (TAB_KEYS[e.key] && !isTextInput(e.target)) {
    e.preventDefault();
    activateRailTab(TAB_KEYS[e.key]);
  }
}

function dispatchTo(name, event) {
  const set = handlers.get(name);
  if (!set) { return; }
  for (const fn of Array.from(set)) {
    try { fn(event); } catch (e) { console.error(`bridge-events handler for ${name} threw`, e); }
  }
}

export function onBridgeEvent(name, fn) {
  const set = handlers.get(name);
  if (!set) { throw new Error(`unknown bridge event: ${name}`); }
  set.add(fn);
  return () => set.delete(fn);
}

export function initBridgeEvents() {
  document.addEventListener("keydown", handleKeydown);
  for (const name of FORWARDED_EVENTS) {
    document.addEventListener(name, (e) => dispatchTo(name, e));
  }
}
