import { initGateway, completeSetupRequest } from "./setup/gateway.js?t=__TOKEN__";
import { setSetupStep } from "./setup/mode.js?t=__TOKEN__";

export { connectFromSetup, editSetupPat } from "./setup/gateway.js?t=__TOKEN__";
export { setSetupStep, openSetupMode, closeSetupMode, applySetupMode } from "./setup/mode.js?t=__TOKEN__";
export { renderSetupAgents } from "./setup/agents.js?t=__TOKEN__";

export const completeSetup = () => {
  completeSetupRequest();
  setSetupStep("done");
};

export const initSetup = () => {
  initGateway();
};
