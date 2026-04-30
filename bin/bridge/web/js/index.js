import { initEvents } from "./events/registry.js?t=__TOKEN__";
import { initKeyboard } from "./events/keyboard.js?t=__TOKEN__";
import { initTabs } from "./tabs.js?t=__TOKEN__";
import { initSetup, applySetupMode } from "./setup.js?t=__TOKEN__";
import { initMarketplace, maybeRefreshMarketplace, renderMarketplaceBadge } from "./marketplace.js?t=__TOKEN__";
import { subscribePolling, subscribeLog } from "./state.js?t=__TOKEN__";
import { append, initToast, syncToastFromState } from "./drawer.js?t=__TOKEN__";
import { renderCloud } from "./cloud.js?t=__TOKEN__";
import { renderProxy } from "./proxy.js?t=__TOKEN__";
import { renderHosts } from "./hosts.js?t=__TOKEN__";
import { renderAgentPresence, renderAgentsSummary, renderAgentsRailCount } from "./agents.js?t=__TOKEN__";
import { renderOverallBadge } from "./overall-badge.js?t=__TOKEN__";
import { renderSyncPill } from "./sync-pill.js?t=__TOKEN__";
import { renderProfile } from "./profile.js?t=__TOKEN__";
import { renderFooter } from "./footer.js?t=__TOKEN__";
import { syncRailIndicator } from "./rail-indicator.js?t=__TOKEN__";

function applySnapshot(snap) {
  renderCloud(snap);
  renderProxy(snap);
  renderHosts(snap);
  renderAgentPresence(snap);
  renderAgentsSummary(snap);
  renderAgentsRailCount(snap);
  renderOverallBadge(snap);
  renderMarketplaceBadge(snap);
  renderSyncPill(snap);
  renderProfile(snap);
  renderFooter(snap);
  applySetupMode(snap);
  maybeRefreshMarketplace(snap);
  syncToastFromState(snap);
  requestAnimationFrame(syncRailIndicator);
}

initEvents();
initKeyboard();
initTabs();
initSetup();
initMarketplace();
initToast();
subscribePolling(applySnapshot);
subscribeLog(append);
