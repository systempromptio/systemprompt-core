export const $ = (id) => document.getElementById(id);

export function setDot(el, cls) {
  if (!el) return;
  el.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err", "dot-warn");
  el.classList.add(cls);
}

export function setBadge(el, text, cls) {
  if (!el) return;
  el.textContent = text;
  el.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
  el.classList.add(cls);
}

export function fmtRelative(unix) {
  if (!unix) return "never";
  const delta = Math.max(0, Math.floor(Date.now() / 1000) - unix);
  if (delta < 5) return "just now";
  if (delta < 60) return `${delta}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  return `${Math.floor(delta / 3600)}h ago`;
}
