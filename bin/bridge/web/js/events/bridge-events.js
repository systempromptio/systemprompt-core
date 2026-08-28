import { TAB_KEYS, isTextInput } from "/assets/js/utils/rail-tabs.js";

const FORWARDED_EVENTS = ["mkt:count", "setup-open", "sp:toast"];
const handlers = new Map();
for (const name of FORWARDED_EVENTS) { handlers.set(name, new Set()); }

function activateRailTab(name, options) {
  const rail = document.querySelector("sp-rail");
  if (rail && typeof rail.activateTab === "function") {
    rail.activateTab(name, options);
    return true;
  }
  return false;
}

// Why: the search box only exists while the Marketplace pane is rendered, so on
// every other pane this used to swallow the keystroke and do nothing. Switching
// to the pane first makes the shortcut mean the same thing everywhere it is
// advertised.
function focusSearch() {
  // Why: hidden panels keep their DOM, so testing for the input alone would
  // "succeed" by focusing a field nobody can see -- which is the shape of the
  // original bug, just moved.
  const panel = document.getElementById("sp-panel-marketplace");
  if (!panel) { return false; }
  const focusInput = () => {
    const search = document.getElementById("mkt-search");
    if (!search) { return false; }
    search.focus();
    search.select();
    return true;
  };
  if (!panel.hidden) { return focusInput(); }
  if (!activateRailTab("marketplace")) { return false; }
  requestAnimationFrame(focusInput);
  return true;
}

function handleKeydown(e) {
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) { return; }
  if (e.key === "f" && !isTextInput(e.target)) {
    if (focusSearch()) { e.preventDefault(); }
    return;
  }
  if (TAB_KEYS[e.key] && !isTextInput(e.target)) {
    e.preventDefault();
    activateRailTab(TAB_KEYS[e.key], { moveFocus: true });
  }
}

// Why: index.html marks cross-pane links with `data-jump-tab` and nothing has
// ever handled the attribute, so the Status pane's link to Agents was inert.
function handleJumpClick(e) {
  const trigger = e.target instanceof Element
    ? e.target.closest("[data-jump-tab]")
    : null;
  if (!trigger) { return; }
  e.preventDefault();
  activateRailTab(trigger.dataset.jumpTab, { moveFocus: true });
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
  document.addEventListener("click", handleJumpClick);
  for (const name of FORWARDED_EVENTS) {
    document.addEventListener(name, (e) => dispatchTo(name, e));
  }
}
