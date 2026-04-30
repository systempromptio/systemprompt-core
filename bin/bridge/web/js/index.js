import "/assets/js/components/base.js?t=__TOKEN__";
import "./theme.js?t=__TOKEN__";
import { init as initI18n } from "./i18n.js?t=__TOKEN__";

initI18n().catch((e) => console.warn("i18n init failed", e));
import "/assets/js/components/sp-cloud-status.js?t=__TOKEN__";
import { bridge } from "/assets/js/bridge.js?t=__TOKEN__";
import {
  gatewayAtom, identityAtom, signedInAtom, agentsOnboardedAtom,
} from "/assets/js/atoms.js?t=__TOKEN__";
import { initEvents } from "./events/registry.js?t=__TOKEN__";
import { initKeyboard } from "./events/keyboard.js?t=__TOKEN__";
import { initTabs } from "./tabs.js?t=__TOKEN__";
import { initSetup, applySetupMode } from "./setup.js?t=__TOKEN__";
import { initMarketplace, maybeRefreshMarketplace, renderMarketplaceBadge } from "./marketplace.js?t=__TOKEN__";
import { subscribePolling, subscribeLog } from "./state.js?t=__TOKEN__";
import { append, initToast, syncToastFromState } from "./drawer.js?t=__TOKEN__";
import { renderProxy } from "./proxy.js?t=__TOKEN__";
import { renderHosts } from "./hosts.js?t=__TOKEN__";
import { renderAgentPresence, renderAgentsSummary, renderAgentsRailCount } from "./agents.js?t=__TOKEN__";
import { renderOverallBadge } from "./overall-badge.js?t=__TOKEN__";
import { renderSyncPill } from "./sync-pill.js?t=__TOKEN__";
import { renderProfile } from "./profile.js?t=__TOKEN__";
import { renderFooter } from "./footer.js?t=__TOKEN__";
import { syncRailIndicator } from "./rail-indicator.js?t=__TOKEN__";

function applySnapshot(snap) {
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

bridge.subscribe("state.changed", (snap) => {
  gatewayAtom.value         = snap.gateway_status;
  identityAtom.value        = snap.verified_identity;
  signedInAtom.value        = !!snap.signed_in;
  agentsOnboardedAtom.value = !!snap.agents_onboarded;
});
bridge.stateSnapshot().then((snap) => {
  gatewayAtom.value         = snap.gateway_status;
  identityAtom.value        = snap.verified_identity;
  signedInAtom.value        = !!snap.signed_in;
  agentsOnboardedAtom.value = !!snap.agents_onboarded;
}).catch((e) => console.error("initial state snapshot failed", e));

initEvents();
initKeyboard();
initTabs();
initSetup();
initMarketplace();
initToast();
subscribePolling(applySnapshot);
subscribeLog(append);
