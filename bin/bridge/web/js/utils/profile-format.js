import { t } from "/assets/js/i18n.js";

export function fmtCompactNumber(n) {
  if (n == null) { return "—"; }
  const v = Number(n);
  if (!Number.isFinite(v)) { return "—"; }
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(2)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

export function fmtUsdMicros(microdollars) {
  if (microdollars == null) { return "—"; }
  const usd = Number(microdollars) / 1_000_000;
  if (!Number.isFinite(usd)) { return "—"; }
  if (usd >= 100) { return `$${usd.toFixed(0)}`; }
  if (usd >= 1) { return `$${usd.toFixed(2)}`; }
  if (usd >= 0.01) { return `$${usd.toFixed(3)}`; }
  return `$${usd.toFixed(5)}`;
}

export function fmtCostDelta(curr, prev) {
  if (curr == null || prev == null || Number(prev) === 0) { return ""; }
  const pct = ((Number(curr) - Number(prev)) / Number(prev)) * 100;
  if (!Number.isFinite(pct)) { return ""; }
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct.toFixed(0)}% vs prev`;
}

export function fmtIsoRelative(iso) {
  if (!iso) { return "—"; }
  const parsed = Date.parse(iso);
  if (!Number.isFinite(parsed)) { return "—"; }
  const diffSec = Math.floor((Date.now() - parsed) / 1000);
  if (diffSec < 60) { return `${diffSec}s ago`; }
  if (diffSec < 3600) { return `${Math.floor(diffSec / 60)}m ago`; }
  if (diffSec < 86400) { return `${Math.floor(diffSec / 3600)}h ago`; }
  return `${Math.floor(diffSec / 86400)}d ago`;
}

export function fmtUnixUtc(unix) {
  if (!unix) { return "—"; }
  const ms = Number(unix) * 1000;
  if (!Number.isFinite(ms)) { return "—"; }
  return `${new Date(ms).toISOString().replace("T", " ").slice(0, 19)} UTC`;
}

export function decodeJwtClaims(token) {
  if (!token || typeof token !== "string") { return null; }
  const parts = token.split(".");
  if (parts.length !== 3) { return null; }
  try {
    const padded = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    return JSON.parse(atob(padded + "===".slice((padded.length + 3) % 4)));
  } catch (_) {
    return null;
  }
}

/* Rows for whatever a brand's own whoami sent that core has no name for.
 *
 * A deployment can point the bridge at its own identity endpoint (see
 * `gateway::identity_source`); the keys it answers with beyond core's six
 * arrive here untouched. The key is the label — humanised, not translated —
 * so the server side is expected to name its fields in words, and the
 * response is expected to be flat: a nested object has no honest one-line
 * rendering and is skipped rather than printed as `[object Object]`.
 *
 * A `_unix` suffix means "this is a timestamp"; the suffix is dropped from
 * the label and the value formatted as a local date.
 */
function humaniseKey(key) {
  const words = key.replace(/_unix$/, "").replace(/_/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

function extraValue(key, value) {
  if (value == null || value === "") { return null; }
  if (Array.isArray(value)) {
    const items = value.filter((v) => v != null && typeof v !== "object");
    return items.length ? items.join(", ") : null;
  }
  if (typeof value === "boolean") { return value ? "Yes" : "No"; }
  if (typeof value === "object") { return null; }
  if (/_unix$/.test(key) && Number.isFinite(Number(value))) {
    const ms = Number(value) * 1000;
    return Number.isFinite(ms) ? new Date(ms).toLocaleString() : null;
  }
  return String(value);
}

export function profileExtraRows(extra) {
  if (!extra || typeof extra !== "object" || Array.isArray(extra)) { return []; }
  return Object.entries(extra)
    .map(([k, v]) => [humaniseKey(k), extraValue(k, v)])
    .filter(([, v]) => v != null);
}

export function profileInitials(idSrc) {
  const letters = (idSrc || "").replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
  return letters || "SP";
}

// The subtitle doubles as the update progress line; when nothing is
// happening it falls back to the usual `tenant · version`.
export function railProfileSubtitle(update, baseVersion, tenant) {
  // Why: only the phases that have a line carry a key; every other phase
  // falls through to the version line. The update payload is the arguments.
  const line = (update.in_progress || update.tone === "err") ? t(`update-phase-${update.phase}`, update) : "";
  if (line) { return line; }
  return tenant ? `${tenant} · ${baseVersion}` : baseVersion;
}
