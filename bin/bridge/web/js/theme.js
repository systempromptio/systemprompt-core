import { subscribe } from "./bridge.js";

const lightQ = window.matchMedia("(prefers-color-scheme: light)");
const contrastQ = window.matchMedia("(prefers-contrast: more)");

function apply() {
  const elevated = lightQ.matches || contrastQ.matches;
  document.documentElement.dataset.contrast = elevated ? "elevated" : "default";
}

lightQ.addEventListener("change", apply);
contrastQ.addEventListener("change", apply);

subscribe("os.theme-changed", apply);

apply();
