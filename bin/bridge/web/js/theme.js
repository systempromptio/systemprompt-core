import { subscribe } from "./bridge.js";

// Colour scheme and contrast are independent: they are two attributes, each
// with a stored user override above the OS preference, so a machine with
// neither OS setting still leaves the user a choice.

const STORAGE_THEME = "bridge.theme";

const darkQ = window.matchMedia("(prefers-color-scheme: dark)");
const contrastQ = window.matchMedia("(prefers-contrast: more)");

function stored(key) {
  try { return window.localStorage.getItem(key); } catch (_) { return null; }
}

function store(key, value) {
  try {
    if (value === "system") { window.localStorage.removeItem(key); }
    else { window.localStorage.setItem(key, value); }
  } catch (_) { /* private mode: the preference is simply not remembered */ }
}

// Why: a brand that ships one dark palette pins the GUI dark (Brand::force_dark,
// injected into <head> before this module loads). Answering here rather than in
// apply() means the stored override, the OS query and the settings UI all agree
// there is only one theme, instead of offering a choice that does nothing.
export function forcedDark() { return window.__SP_FORCE_DARK__ === true; }

export function themePreference() {
  if (forcedDark()) { return "dark"; }
  return stored(STORAGE_THEME) || "system";
}

function apply() {
  const theme = themePreference();
  const root = document.documentElement;
  root.dataset.theme = theme === "system" ? (darkQ.matches ? "dark" : "light") : theme;
  root.dataset.contrast = contrastQ.matches ? "elevated" : "default";
}

export function setTheme(value) { store(STORAGE_THEME, value); apply(); }

export function initTheme() {
  darkQ.addEventListener("change", apply);
  contrastQ.addEventListener("change", apply);
  subscribe("os.theme-changed", apply);
  apply();
}
