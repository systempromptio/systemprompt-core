export const $ = (id) => document.getElementById(id);

export function setDot(el, cls) {
  if (el) {
    el.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err", "dot-warn");
    el.classList.add(cls);
  }
}

export function setBadge(el, text, cls) {
  if (el) {
    el.textContent = text;
    el.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
    el.classList.add(cls);
  }
}

export function fmtRelative(unix) {
  if (!unix) {
    return "never";
  }
  const delta = Math.max(0, Math.floor(Date.now() / 1000) - unix);
  if (delta < 5) {
    return "just now";
  } else if (delta < 60) {
    return `${delta}s ago`;
  } else if (delta < 3600) {
    return `${Math.floor(delta / 60)}m ago`;
  } else {
    return `${Math.floor(delta / 3600)}h ago`;
  }
}

export function fillTemplate(id, slots) {
  const tpl = $(id);
  if (!tpl) {
    return document.createDocumentFragment();
  }
  const frag = tpl.content.cloneNode(true);
  if (slots) {
    for (const [selector, value] of Object.entries(slots)) {
      const el = frag.querySelector(selector);
      if (el) {
        if (value && typeof value === "object" && "text" in value) {
          el.textContent = value.text;
        } else if (value && typeof value === "object" && "attrs" in value) {
          for (const [k, v] of Object.entries(value.attrs)) {
            el.setAttribute(k, v);
          }
        } else {
          el.textContent = value;
        }
      }
    }
  }
  return frag;
}
