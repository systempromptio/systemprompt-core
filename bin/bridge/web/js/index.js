import "/assets/js/components/base.js?t=__TOKEN__";
import "./theme.js?t=__TOKEN__";
import { init as initI18n } from "./i18n.js?t=__TOKEN__";

initI18n().catch((e) => console.warn("i18n init failed", e));

import "/assets/js/components/sp-cloud-status.js?t=__TOKEN__";
import "/assets/js/components/sp-proxy-status.js?t=__TOKEN__";
import "/assets/js/components/sp-agent-presence.js?t=__TOKEN__";
import "/assets/js/components/sp-agents-summary.js?t=__TOKEN__";
import "/assets/js/components/sp-overall-badge.js?t=__TOKEN__";
import "/assets/js/components/sp-sync-pill.js?t=__TOKEN__";
import "/assets/js/components/sp-rail-profile.js?t=__TOKEN__";
import "/assets/js/components/sp-footer.js?t=__TOKEN__";
import "/assets/js/components/sp-crumb.js?t=__TOKEN__";
import "/assets/js/components/sp-rail.js?t=__TOKEN__";
import "/assets/js/components/sp-toast.js?t=__TOKEN__";
import "/assets/js/components/sp-activity-log.js?t=__TOKEN__";
import "/assets/js/components/sp-host-card.js?t=__TOKEN__";
import "/assets/js/components/sp-hosts-list.js?t=__TOKEN__";
import "/assets/js/components/sp-settings.js?t=__TOKEN__";
import "/assets/js/components/sp-marketplace.js?t=__TOKEN__";
import "/assets/js/components/sp-marketplace-list.js?t=__TOKEN__";
import "/assets/js/components/sp-marketplace-detail.js?t=__TOKEN__";
import "/assets/js/components/sp-setup.js?t=__TOKEN__";
import "/assets/js/components/sp-setup-gateway.js?t=__TOKEN__";
import "/assets/js/components/sp-setup-agents.js?t=__TOKEN__";

import { bridge } from "/assets/js/bridge.js?t=__TOKEN__";
import {
  gatewayAtom, identityAtom, signedInAtom, agentsOnboardedAtom,
} from "/assets/js/atoms.js?t=__TOKEN__";

function hydrateAtoms(snap) {
  if (!snap) { return; }
  gatewayAtom.value         = snap.gateway_status;
  identityAtom.value        = snap.verified_identity;
  signedInAtom.value        = !!snap.signed_in;
  agentsOnboardedAtom.value = !!snap.agents_onboarded;
}

bridge.subscribe("state.changed", hydrateAtoms);
bridge.stateSnapshot().then(hydrateAtoms).catch((e) => console.error("initial state snapshot failed", e));
