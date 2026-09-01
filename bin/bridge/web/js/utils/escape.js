export function escapeHtml(s) {
  if (s == null) { return ""; }
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function attr(name, value) {
  if (value == null || value === false) { return ""; }
  if (value === true) { return name; }
  return `${name}="${escapeHtml(value)}"`;
}
