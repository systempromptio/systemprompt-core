import { $ } from "./dom.js?t=__TOKEN__";
import { api } from "./api.js?t=__TOKEN__";

const LOG_POLL_MS = 1000;
let logCursor = 0;

export function append(line) {
  const log = $("log");
  if (!log) return;
  const ts = new Date().toLocaleTimeString();
  log.textContent += `\n[${ts}] ${line}`;
  log.scrollTop = log.scrollHeight;
}

export async function pollLog() {
  try {
    const resp = await api(`/api/log?since=${logCursor}`);
    if (resp.ok) {
      const entries = await resp.json();
      for (const e of entries) {
        append(e.line);
        if (e.id > logCursor) logCursor = e.id;
      }
    }
  } catch (e) {
    console.error("log poll failed", e);
  } finally {
    setTimeout(pollLog, LOG_POLL_MS);
  }
}
