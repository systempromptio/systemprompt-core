import { subscribe } from "./bridge.js";

// Colour scheme and contrast are independent, and this file used to collapse
// them: `prefers-color-scheme: light || prefers-contrast: more` both set
// data-contrast="elevated", so asking your OS for light mode handed you a
// *darker* high-contrast dark theme and no light theme existed. They are now two
// attributes, each with a stored user override above the OS preference, because
// a machine with neither OS setting left the user no recourse at all.

const STORAGE_THEME = "bridge.theme";
const STORAGE_CONTRAST = "bridge.contrast";

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

export function themePreference() { return stored(STORAGE_THEME) || "system"; }
export function contrastPreference() { return stored(STORAGE_CONTRAST) || "system"; }

function apply() {
  const theme = themePreference();
  const contrast = contrastPreference();
  const root = document.documentElement;
  root.dataset.theme = theme === "system" ? (darkQ.matches ? "dark" : "light") : theme;
  root.dataset.contrast = contrast === "system"
    ? (contrastQ.matches ? "elevated" : "default")
    : contrast;
}

export function setTheme(value) { store(STORAGE_THEME, value); apply(); }
export function setContrast(value) { store(STORAGE_CONTRAST, value); apply(); }

export function initTheme() {
  darkQ.addEventListener("change", apply);
  contrastQ.addEventListener("change", apply);
  subscribe("os.theme-changed", apply);
  apply();
}
