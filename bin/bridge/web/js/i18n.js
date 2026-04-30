// Minimal Fluent-subset loader. Supports `id = value` lines and `{ $arg }`
// substitution. Comments start with `#`. Unknown keys fall back to the
// element's existing text content (which is the en-US literal hard-coded
// in the HTML, so the UI degrades gracefully).

const messages = new Map();
let activeLocale = "en-US";

function parseFtl(src) {
  const out = new Map();
  for (const raw of src.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const id = line.slice(0, eq).trim();
    const value = line.slice(eq + 1).trim();
    if (id) out.set(id, value);
  }
  return out;
}

function format(template, args) {
  return template.replace(/\{\s*\$([A-Za-z0-9_-]+)\s*\}/g, (_, name) => {
    if (args && Object.prototype.hasOwnProperty.call(args, name)) {
      return String(args[name]);
    }
    return "";
  });
}

export function t(id, args) {
  const msg = messages.get(id);
  if (typeof msg !== "string") return id;
  return args ? format(msg, args) : msg;
}

export function hydrate(root = document) {
  for (const el of root.querySelectorAll("[data-l10n-id]")) {
    const id = el.dataset.l10nId;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.textContent = msg;
    }
  }
  for (const el of root.querySelectorAll("[data-l10n-placeholder]")) {
    const id = el.dataset.l10nPlaceholder;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.placeholder = msg;
    }
  }
  for (const el of root.querySelectorAll("[data-l10n-aria]")) {
    const id = el.dataset.l10nAria;
    const msg = messages.get(id);
    if (typeof msg === "string") {
      el.setAttribute("aria-label", msg);
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
  hydrate();
}

export function locale() {
  return activeLocale;
}
