const ESCAPES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export const escapeHtml = (value) => String(value ?? "").replace(/[&<>"']/g, (c) => ESCAPES[c]);

export const attr = (name, value) => {
  if (value == null || value === false) { return ""; }
  if (value === true) { return ` ${name}`; }
  return ` ${name}="${escapeHtml(value)}"`;
};

const UNITS = ["B", "KB", "MB", "GB"];

export const formatBytes = (bytes) => {
  if (bytes == null) { return ""; }
  let size = Number(bytes);
  let unit = 0;
  while (size >= 1024 && unit < UNITS.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${UNITS[unit]}`;
};

export const formatTime = (iso) => {
  if (!iso) { return ""; }
  const date = new Date(iso);
  return Number.isNaN(date.valueOf()) ? "" : date.toLocaleString();
};

export const errorText = (err) => (err?.code ? `${err.code}: ${err.message}` : String(err?.message ?? err));
