import { apiGet } from "./api.js?t=__TOKEN__";

const STATE_POLL_MS = 1500;
const LOG_POLL_MS = 1000;
let logCursor = 0;

export function subscribePolling(applySnapshot) {
  const tick = async () => {
    try {
      const snap = await apiGet("/api/state");
      applySnapshot(snap);
    } catch (e) {
      console.error("state poll failed", e);
    } finally {
      setTimeout(tick, STATE_POLL_MS);
    }
  };
  tick();
}

export function subscribeLog(append) {
  const tick = async () => {
    try {
      const entries = await apiGet(`/api/log?since=${logCursor}`);
      for (const e of entries) {
        append(e.line);
        if (e.id > logCursor) {
          logCursor = e.id;
        }
      }
    } catch (e) {
      console.error("log poll failed", e);
    } finally {
      setTimeout(tick, LOG_POLL_MS);
    }
  };
  tick();
}
