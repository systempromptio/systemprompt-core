import { t } from "/assets/js/i18n.js";

export const LOG_LEVELS = ["all", "warn", "error"];

export function fmtLogCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(1)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

export function fmtLogClock(tsUnix) {
  if (!tsUnix) { return "--:--:--"; }
  return new Date(tsUnix * 1000).toLocaleTimeString();
}

// Rust stamps the entry when it happens; deriving it from arrival time mislabelled
// anything queued, batched or replayed — which is every backfilled line.
export function toLogEntry(record) {
  const line = record && record.line ? record.line : String(record ?? "");
  const level = (record && record.level) || "info";
  return { text: `[${fmtLogClock(record && record.ts_unix)}] ${line}`, level, meta: { line, level } };
}

export function logLevelLabel(level) {
  if (level === "all") { return t("activity-level-all") || "All"; }
  if (level === "warn") { return t("activity-level-warn") || "Warnings"; }
  return t("activity-level-error") || "Errors";
}

export function logFilterFor(query, level) {
  const q = (query || "").trim().toLowerCase();
  if (!q && level === "all") { return null; }
  return (entry) => {
    if (level === "warn" && entry.level === "info") { return false; }
    if (level === "error" && entry.level !== "error") { return false; }
    return !q || entry.text.toLowerCase().includes(q);
  };
}
