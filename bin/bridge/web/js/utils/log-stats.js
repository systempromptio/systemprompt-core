import { fmtLogCount } from "/assets/js/utils/log-format.js";

export function proxyStatCells(snap) {
  const stats = (snap && snap.proxy_stats) || {};
  return [
    ["msgs", fmtLogCount(stats.messages_total)],
    ["tin", fmtLogCount(stats.tokens_in_total)],
    ["tout", fmtLogCount(stats.tokens_out_total)],
  ];
}

export function trimToCapacity(entries, capacity) {
  if (entries.length > capacity) {
    entries.splice(0, entries.length - capacity);
  }
  return entries;
}
