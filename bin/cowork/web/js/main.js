import { api } from "./api.js?t=__TOKEN__";
import { initTabs } from "./tabs.js?t=__TOKEN__";
import { initSetup } from "./setup.js?t=__TOKEN__";
import { pollLog, append } from "./activity.js?t=__TOKEN__";
import { applySnapshot, onSnapshot } from "./snapshot.js?t=__TOKEN__";
import { initMarketplace, maybeRefreshMarketplace } from "./marketplace.js?t=__TOKEN__";
import { post } from "./api.js?t=__TOKEN__";
import { $ } from "./dom.js?t=__TOKEN__";

const STATE_POLL_MS = 1500;

function wireGlobalActions() {
  $("btn-logout").addEventListener("click",          () => post("/api/logout",      null, append));
  $("btn-sync").addEventListener("click",            () => post("/api/sync",        null, append));
  $("btn-validate").addEventListener("click",        () => post("/api/validate",    null, append));
  $("btn-open-folder").addEventListener("click",     () => post("/api/open_folder", null, append));
  $("btn-recheck").addEventListener("click",         () => post("/api/probe",       null, append));
  $("btn-settings-folder")?.addEventListener("click",         () => post("/api/open_folder", null, append));
  $("btn-settings-validate")?.addEventListener("click",       () => post("/api/validate",    null, append));
  $("btn-settings-change-gateway")?.addEventListener("click", () => document.body.classList.add("setup-mode"));
}

async function pollState() {
  try {
    const resp = await api("/api/state");
    if (resp.ok) {
      const snap = await resp.json();
      applySnapshot(snap);
    }
  } catch (e) {
    console.error("state poll failed", e);
  } finally {
    setTimeout(pollState, STATE_POLL_MS);
  }
}

initTabs();
initSetup();
initMarketplace();
wireGlobalActions();
onSnapshot(maybeRefreshMarketplace);
pollState();
pollLog();
