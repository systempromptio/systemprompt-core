import { parseFtl, resolveSelects } from "/assets/js/i18n-ftl.js";

const messages = new Map();
let activeLocale = "en-US";
let ready = false;

function warnMissing(id, where) {
  if (ready) console.warn(`i18n: no message "${id}" for ${where}`);
}

function format(template, args) {
  return resolveSelects(template, args).replace(/\{\s*\$([A-Za-z0-9_-]+)\s*\}/g, (_, name) => {
    if (args && Object.prototype.hasOwnProperty.call(args, name)) {
      return String(args[name]);
    }
    return "";
  });
}

export function t(id, args) {
  const msg = messages.get(id);
  // Returning the id here made every `|| "English"` fallback in the app
  // unreachable, so a missing key rendered its own id at the user.
  if (typeof msg !== "string") return undefined;
  return args ? format(msg, args) : msg;
}

export function hydrate(root = document) {
  for (const el of root.querySelectorAll("[data-l10n-id]")) {
    const id = el.dataset.l10nId;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.textContent = msg;
    } else {
      warnMissing(id, "data-l10n-id");
    }
  }
  for (const el of root.querySelectorAll("[data-l10n-placeholder]")) {
    const id = el.dataset.l10nPlaceholder;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.placeholder = msg;
    } else {
      warnMissing(id, "data-l10n-placeholder");
    }
  }
  for (const el of root.querySelectorAll("[data-l10n-aria]")) {
    const id = el.dataset.l10nAria;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.setAttribute("aria-label", msg);
    } else {
      warnMissing(id, "data-l10n-aria");
    }
  }
}

async function loadCatalog(locale) {
  try {
    const res = await fetch(`/assets/i18n/${locale}/bridge.ftl`);
    if (!res.ok) return null;
    return parseFtl(await res.text());
  } catch (_) {
    return null;
  }
}

export async function init() {
  const requested = (navigator.language || "en-US").replace("_", "-");
  const fallback = "en-US";
  const fallbackCatalog = await loadCatalog(fallback);
  if (fallbackCatalog) {
    for (const [k, v] of fallbackCatalog) messages.set(k, v);
  }
  if (requested !== fallback) {
    const localeCatalog = await loadCatalog(requested);
    if (localeCatalog) {
      activeLocale = requested;
      for (const [k, v] of localeCatalog) messages.set(k, v);
    }
  }
  ready = true;
  hydrate();
  // Custom elements upgrade synchronously on import, so any `t()` baked into a
  // first render resolved against an empty catalog and emitted the raw message
  // id. `hydrate()` only patches [data-l10n-id] nodes, not innerHTML built by
  // render() — so tell components to render again now the catalog is in.
  document.dispatchEvent(new CustomEvent("sp-i18n-ready"));
}

export function isReady() {
  return ready;
}

export function locale() {
  return activeLocale;
}
