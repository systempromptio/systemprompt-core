import { invoke } from "/assets/js/bridge.js";

// On macOS the window's titlebar is transparent and the webview fills it, so
// AppKit never sees a click it could turn into a window move. The topbar, and
// the titlebar strip above the setup screen, stand in for it: a press there
// asks the native side to start the drag, a double-click zooms.
const TITLEBAR_HEIGHT = 28;
const DRAG_SURFACE = ".sp-topbar, [data-window-drag]";
const INTERACTIVE = [
  "a", "button", "input", "select", "textarea", "label", "summary",
  "[role='button']", "[role='menu']", "[role='menuitem']", "[role='tab']",
  "[contenteditable]", "[data-no-window-drag]", "sp-topbar-menu",
].join(", ");

function isInteractive(path) {
  return path.some((el) => el instanceof Element && el.matches(INTERACTIVE));
}

function onDragSurface(event, path) {
  if (event.clientY <= TITLEBAR_HEIGHT) { return true; }
  return path.some((el) => el instanceof Element && el.matches(DRAG_SURFACE));
}

function handle(event) {
  if (event.button !== 0) { return; }
  const path = event.composedPath();
  if (isInteractive(path) || !onDragSurface(event, path)) { return; }
  event.preventDefault();
  const cmd = event.detail === 2 ? "window.toggleMaximize" : "window.drag";
  invoke(cmd).catch((e) => console.warn(`${cmd} failed`, e));
}

export function initWindowDrag() {
  if (!document.body.classList.contains("is-platform-macos")) { return; }
  document.addEventListener("mousedown", handle);
}
