# systemprompt-bridge en-US message catalog.
# Add new locale files at web/i18n/<locale>/bridge.ftl. Fall back to en-US.

ready = Ready

# Setup wizard ----------------------------------------------------------------
setup-heading = Welcome to systemprompt bridge
setup-lede = systemprompt bridge routes one or more coding agents through your enterprise gateway. Connect this device, pick the agents you want governed, and you're done.
setup-eyebrow-prefix = DEMO BUILD
setup-gateway-label = Gateway URL
setup-gateway-empty = enter a URL to check…
setup-connect = Connect
setup-sign-in-default = Sign in to your gateway
setup-sign-in-hint = Opens your browser to sign in on the gateway; this device is linked automatically.
setup-keep-signed-in = Keep me signed in on this device
setup-pat-summary = Use a personal access token instead
setup-pat-open-login = Open the gateway login →
setup-pat-edit = Edit
setup-gateway-placeholder = http://127.0.0.1:8080
setup-pat-placeholder = sp-live-…
setup-token-rejected = The gateway rejected that personal access token. Issue a fresh one and try again.
setup-gateway-unreachable-reason = Gateway unreachable: { $reason }
setup-pat-label = Personal access token
setup-pat-hint = Don't have one yet?
setup-agents-lede = Pick the coding agents you want systemprompt bridge to govern. You can install more later from the Agents tab.
setup-finish = Finish
setup-warning-strong = Demo software.
setup-warning-body = This build of systemprompt bridge is provided for demonstration purposes only and is not licensed for production use.
setup-purge-summary = Remove everything from this computer
setup-purge-explainer = Removes the bridge's installed plugins, scheduled sync, managed profile, sign-in, identity and every saved setting. The app returns to this screen as if it had never been set up.
setup-purge-button = Remove everything
setup-purge-confirm = Remove everything the bridge installed on this computer? This cannot be undone.
setup-purge-confirm-button = Yes, remove it all
setup-purge-cancel = Keep it
setup-purge-working = Removing…
toast-purge-failed = Could not remove everything

# Sync / actions --------------------------------------------------------------
sync-button = Sync now
sync-cancel = Cancel
sync-cancelled = Sync cancelled.
login-cancelled = Sign-in cancelled.
login-failure = Sign-in failed: { $error }
sync-failure = Sync failed: { $error }
sync-no-credentials = Sync failed: not signed in. Sign in to this gateway, then try again.
sync-gateway-unauthorized = Sync failed: { $gateway } rejected a freshly issued credential (HTTP { $status } from { $endpoint }). Your access may have been revoked — sign in again, or ask an administrator to check your account.
sync-bridge-too-old = Sync failed: this bridge is { $local } but the gateway requires { $required } or newer. Updating now — the app will restart.
sync-reauthenticate = Re-authenticate

gateway-set-empty = Enter a gateway URL.
gateway-set-failure = Could not save the gateway: { $error }
gateway-set-cancelled = Gateway not saved — cancelled.
gateway-saving = Saving gateway { $url }…
gateway-saved = Gateway saved.

# Sign-in ---------------------------------------------------------------------
login-pat-empty = Enter a personal access token.
login-saving = Signing in…
login-pull-manifest = Signed in — fetching your plugins…
logout-running = Signing out…
purge-running = Removing everything the bridge installed on this computer…
purge-success = Removed everything. The bridge is back to a fresh install.
purge-failure = Remove everything failed: { $error }
logout-success = Signed out
logout-failure = Could not sign out: { $error }
session-rejected = { $gateway } no longer accepts this bridge's credentials ({ $reason }). Sign in again to resume.
validate-result = { $checks } checks · { $failed } failed · { $warned } warnings
validate-running = Re-checking…

# Gateway ---------------------------------------------------------------------
gateway-unreachable = offline

# Marketplace -----------------------------------------------------------------
marketplace-heading = Marketplace
marketplace-categories = Categories
marketplace-categories-aria = Marketplace categories
marketplace-search-placeholder = Search…
marketplace-cat-plugins = Plugins
marketplace-cat-skills = Skills
marketplace-cat-hooks = Hooks
marketplace-cat-mcp = MCP servers
marketplace-cat-agents = Agents
marketplace-cat-artifacts = Artifacts
marketplace-empty-title = Select an item
marketplace-empty-generic = Nothing here yet
marketplace-empty-plugins = No plugins yet
marketplace-empty-skills = No skills yet
marketplace-empty-hooks = No hooks yet
marketplace-empty-mcp = No MCP servers yet
marketplace-empty-agents = No agents yet
marketplace-empty-artifacts = No artifacts yet
marketplace-empty-never-synced = Sync to pull what your account already has.
marketplace-empty-synced = Your last sync did not include anything of this kind.
marketplace-no-matches = No matches
marketplace-items-aria = Items in this category
marketplace-group-ungrouped = Ungrouped
toast-dismiss = Dismiss
agents-status-group-aria = Bridge status
sync-cancel-aria = Cancel sync
marketplace-error-title = Could not load this list
marketplace-retry = Try again
marketplace-change-installed = New
marketplace-change-updated = Updated
marketplace-change-removed = Removed
marketplace-action-validate = Re-check
marketplace-action-open-folder = Open folder
last-sync-never = never synced
# Only `last-sync` survived this block: every other definition here duplicated
# one above, and Fluent resolves a duplicate to the first, so they were dead
# lines that only produced parser warnings.
last-sync = Last sync: { $summary }

# Agents tab ------------------------------------------------------------------
agents-heading = Agents
agents-lede = Every agent you add here runs through systemprompt's local proxy, so each request it makes is governed and logged. Any number of them can run at once.
agents-action-add = Add agent
agents-action-reverify-all = Re-check all
agents-status-cloud-signed-in = signed in as { $email }
agents-status-cloud-signed-out = signed out
agents-status-proxy-listening = Listening · { $latency }ms · { $status }
agents-status-proxy-refused = proxy refused
agents-status-proxy-timeout = proxy timed out
agents-status-proxy-http-error = proxy http error
agents-status-proxy-unconfigured = proxy unconfigured
agents-status-token-expiring = Session expires in { $ttl }
agents-status-token-missing = no token
host-action-open = Open

# Agents tab — plain-language state, actions, drawer -------------------------
agent-state-working = Working
agent-state-ready = Ready
agent-state-attention = Needs attention
agent-state-not-set-up = Not set up
agent-state-down = Not working
agent-state-checking = Checking…
agent-reason-governed = Governed
agent-reason-governed-checked = Governed · checked { $when }
agent-reason-awaiting = Waiting for its first launch
agent-reason-app-missing = The app is not installed on this computer
agent-reason-stale = Its settings are out of date — repair, then restart the app
agent-reason-partial = Some of its settings are missing ({ $missing })
agent-reason-absent = This agent is not routed through systemprompt yet
agent-reason-no-key = No usable model — add an API key for { $providers }
agent-reason-no-models = No model this agent can use is available
agent-reason-proxy-down = The local proxy is not responding
agent-reason-never-probed = Not checked yet
agent-reason-cloud-managed = Managed from the cloud — nothing to install on this computer
agent-action-repair = Repair
agent-action-verify = Re-check
agent-action-add = Add
agent-action-working = Working…
agent-action-open-config = Show config file
agent-action-open = Open
agent-action-download = Download
agent-action-remove = Remove agent
agent-open-details = Open details for { $name }
agent-repair-explainer = Repair rewrites this agent's configuration profile and re-applies it. Restart the agent afterwards.
agent-section-health = Health
agent-section-models = Models
agent-section-config = Technical detail
agent-section-remove = Remove
agent-models-unchecked = This agent's models have not been checked on this computer yet.
agent-remove-explainer = Removing takes this agent's settings back out of its configuration file. It does not uninstall the app.
agent-remove-confirm = Remove { $name } from systemprompt?
agent-remove-confirm-path = Remove { $name } from systemprompt? This strips its systemprompt keys from { $path }.
agent-remove-confirm-button = Remove it
agent-remove-cancel = Keep it
agent-row-profile = Configuration profile
agent-row-app = Application
agent-row-process = Process
agent-row-config-location = Config location
agent-models-count = { $count } models available
agent-model-filter-hint = Saved to your systemprompt account — you must be signed in.
agents-empty-title = No agents set up yet
agents-empty-body = Add a coding agent to route it through systemprompt, so every request it makes is governed and logged.
agents-add-heading = Add an agent
agents-add-lede = Pick a coding agent to route through systemprompt. Adding one writes its configuration profile — you do not need to configure anything by hand.
agents-add-added = Added
agents-add-empty = No agents are available for this installation.
agents-detail-gone = Agent not available
agents-detail-gone-body = This agent is no longer available on this computer.
agents-add-provisional = This list is provisional until this computer has synced with systemprompt.
host-profile-stale = configuration profile out of date — repair to re-apply it
host-resolved-keys = Resolved profile keys
drawer-close = Close

# Status tab ------------------------------------------------------------------
status-heading = Status
status-badge-checking = checking…
status-cloud-heading = systemprompt cloud
status-cloud-caption = The hosted control plane systemprompt bridge talks to. Your identity and personal access token live here.
status-cloud-recheck = Re-check
status-cloud-reach-label = Reachability
status-cloud-identity-label = Identity
status-cloud-logout = Sign out
rail-profile-logout = Sign out
rail-profile-update-cta = Click here to update
rail-profile-update-to = Update to
rail-profile-restart-cta = Restart to finish updating
rail-profile-release-notes = Release notes
status-proxy-heading = Local proxy
status-proxy-caption = The 127.0.0.1 endpoint agents call instead of the Anthropic API.
status-proxy-health = Health
status-proxy-endpoints = Inference endpoints
status-proxy-endpoints-detail = Models the proxy advertises to agents.
status-proxy-endpoints-empty = No models configured yet — start an agent to populate.
status-mcp-heading = MCP servers
status-mcp-caption = Whether managed MCP servers authenticate end-to-end through the proxy, and the tools they expose.
agents-fleet-all-working = all working
agents-fleet-needs-attention = needs attention
agents-fleet-not-working = not working
agents-fleet-checking = checking…
agents-fleet-none-enabled = no agents enabled

# Settings tab ----------------------------------------------------------------
settings-heading = Settings
settings-gateway-label = Gateway URL
settings-plugins-label = Plugins directory
settings-config-label = Config file
settings-schedule-label = Sync schedule
settings-theme-label = Appearance
settings-theme-system = Match my system
settings-theme-light = Light
settings-theme-dark = Dark
settings-action-open-folder = Open config folder
settings-action-validate = Re-check
settings-action-change-gateway = Change gateway
# Composed with the platform name, e.g. "Start with Windows".
settings-licensing-note-prefix = Demo build — for production licensing contact

# Activity drawer / footer ----------------------------------------------------
activity-title = Activity
activity-log-aria = Activity log
activity-totals-aria = Activity totals
activity-help-aria = Help and support
rail-profile-aria = Account and workspace
activity-msgs = msgs
activity-tin = in
activity-tout = out
activity-help-title = Help & Support
activity-shortcuts-title = Keyboard shortcuts
activity-shortcut-search = Search the marketplace
activity-shortcut-escape = Close the open panel
activity-open-log-folder = Open log folder
activity-export-bundle = Export diagnostic bundle
footer-docs = docs
footer-licensing = licensing
footer-tabs-hint = tabs

# Topbar / navigation ---------------------------------------------------------
nav-activity = Activity
nav-account = Account
nav-marketplace = Marketplace
nav-agents = Agents
nav-status = Status
nav-settings = Settings

# Profile tab ----------------------------------------------------------------
profile-heading = Profile
profile-refresh = Refresh
profile-section-identity = Identity
profile-section-usage = Token usage
profile-section-models = Favorite models

# Profile — labels and empty states --------------------------------------------
profile-signed-out = Sign in to see your profile.
profile-usage-empty = No usage reported yet.
profile-models-empty = No model usage in the last 30 days.
profile-conversations-empty = No conversations recorded yet.
profile-loading = loading…
profile-fetch-failed = Could not load your profile.
profile-none = none
profile-window-24h = Last 24 hours
profile-window-7d = Last 7 days
profile-window-30d = Last 30 days
profile-group-by-model = By model
profile-group-by-agent = By agent
profile-group-recent = Recent
profile-id-email = Email
profile-id-name = Name
profile-id-user = User ID
profile-id-tenant = Organization ID
profile-id-provider = Signed in with
profile-id-roles = Roles
profile-id-issuer = Issued by
profile-id-expires = Session expires
profile-id-gateway = Gateway
profile-id-token = Session token
profile-token-value = expires in { $ttl }
profile-plan-auth-scheme = Sign-in method
profile-plan-gateway = Inference gateway
profile-plan-organization = Organization
profile-plan-models = Allowed models
profile-section-conversations = Conversations
profile-section-plan = Plan & gateway
profile-error-fetch = Could not load profile.
nav-section-navigate = Navigate
brand-workspace-pill = bridge workspace

# Marketplace badges / detail / empty -----------------------------------------
marketplace-badge-signin = sign-in required
marketplace-badge-syncing = syncing
marketplace-badge-synced = synced
marketplace-badge-never = never synced
marketplace-detail-contents = Contents
marketplace-detail-readme = README
marketplace-detail-tools = Tools
marketplace-detail-path = Path
marketplace-detail-copy = Copy
marketplace-detail-copied = Copied

# Agents (the coding agents on this computer) ----------------------------------
host-profile-installed = configuration profile installed
host-profile-partial = configuration profile incomplete (missing: { $missing })
host-process-running = running
host-process-not-running = not running
host-jwt-warn = This agent's session expires in { $ttl }. Repair the agent to renew it.
host-prefs-empty = (no keys present)
host-app-installed = app installed
host-app-not-installed = app not installed
host-app-unknown = could not determine
host-action-download = Download
host-missing-keys = Missing required keys
host-last-generated = Last generated
host-profile-uuid = Configuration profile ID
host-payload-uuid = Payload ID
host-kind = Agent kind
host-config-format = Config format
host-install-label = Install action
host-compatible-models = Compatible models
host-no-compatible-models = none available
host-model-filter = Model filter
host-model-filter-all = All models
proto-anthropic = Claude models
proto-openai = OpenAI models
proto-gemini = Gemini models
host-model-filter-custom = custom override
host-model-filter-default = host default
host-model-filter-save = Save filter
host-model-filter-reset = Reset to default
host-model-filter-unsaved = Unsaved changes.

# Agents (tab summary + setup) ------------------------------------------------
setup-agents-progress = { $done } of { $total } agents set up
setup-agents-installed = Installed
setup-agents-install = Install configuration profile
setup-agents-empty = No agents available on this platform.

# Setup gateway probe ---------------------------------------------------------
setup-connecting = Connecting…
setup-signing-in = Waiting for your browser…
setup-sign-in-cancel = Cancel
setup-gateway-required = Check the gateway URL under Advanced, then try again.
setup-step-label-connect = Step 1 of 2
setup-step-label-agents = Step 2 of 2

# Native menu bar ------------------------------------------------------------
menu-edit = Edit
menu-view = View
menu-help = Help
menu-show-settings = Show settings window
menu-open-log-folder = Open log folder
menu-export-bundle = Export diagnostic bundle
menu-open-config = Open config folder

# Tray -------------------------------------------------------------------------
tray-sync-now = Sync now
tray-syncing = Syncing…
tray-validate = Re-check
tray-check-updates = Check for updates
tray-open-settings = Show settings window
tray-open-config = Open config folder
tray-autostart = Start at login
tray-sign-out = Sign out
tray-quit = Quit

# Topbar overflow menu ---------------------------------------------------------
# Windows has no global menu bar, so the Edit/View/Help commands live here.
topbar-menu-label = More actions
topbar-menu-settings = Settings

# Action outcomes -------------------------------------------------------------
toast-agent-repaired = { $name } re-configured — wrote { $path }. Restart { $name } to pick it up.
toast-agent-added = { $name } added — wrote { $path }. Restart { $name } to pick it up.
toast-agent-verified = { $name } re-checked.
toast-agent-removed = { $name } removed. Restart it to drop the old settings.
toast-agent-remove-manual = { $name }: { $instruction }
toast-agent-remove-nothing = { $name } had nothing left to remove.
toast-model-filter-saved = Model filter saved to your systemprompt account.
toast-model-filter-reset = Model filter reset to this agent's default.
toast-sync-started = Sync started.
toast-validate-ok = Configuration validated.
toast-folder-opened = Opened the configuration folder.
toast-log-folder-opened = Opened the log folder.
toast-bundle-exported = Exported the diagnostic bundle.
toast-copied = Copied to the clipboard.
toast-setup-complete-failed = Could not record that setup finished.
toast-update-restarting = Restarting to finish the update…
setup-install-stage-generate = Could not write the profile
setup-install-stage-install = Could not apply the profile
setup-install-stage-probe = Could not re-check the agent

# Setup: settling, finishing, and committing the gateway ----------------------
setup-settle-slow = Still checking this computer. Some agents have not reported yet.
setup-settle-unreachable = Could not reach { $gateway }. Check the URL and that the gateway is running.
setup-retry = Check again
setup-continue-anyway = Continue anyway
setup-update-available = Version { $version } is available for this build.
setup-update-ready = Version { $version } is ready to finish installing.
setup-finish-empty-warning = You have not added an agent yet, so nothing will be routed through systemprompt.
setup-finish-anyway = Finish anyway
setup-gateway-required-url = Enter the gateway URL.
setup-gateway-scheme = Gateway URL must start with http:// or https://
setup-gateway-loopback-https = A gateway on this machine is served over http://, not https:// — drop the "s".
setup-gateway-save-failed = Could not save the gateway URL

# Governance, requests, setup health and settings ------------------------------
activity-bundle-done = Diagnostic bundle written.
activity-bundle-written = Diagnostic bundle written to { $path }
activity-copied = Copied
activity-collapse-line = Collapse
activity-copy = Copy
activity-empty = No activity yet.
activity-level-all = All
activity-level-error = Errors
activity-level-warn = Warnings
activity-search-placeholder = Filter activity…
settings-config-malformed = Your configuration file could not be read, so nothing can be saved until it is fixed: { $malformed }
settings-gateway-cancel = Cancel
settings-gateway-empty = Enter a gateway URL.
settings-gateway-https = The gateway must be https, except on localhost.
settings-gateway-invalid = That is not a valid URL.
settings-gateway-save = Save
settings-mtls-label = Client certificate
settings-mtls-none = Not configured
settings-pin-label = Manifest signing key
settings-pin-none = Not pinned — the first sync will trust and pin whatever key it is served
settings-pin-source-operator = Set on this device
settings-pin-source-policy = Set by device policy
settings-schedule-installed = Registered with the system scheduler as { $label }
settings-schedule-unknown = Could not be determined on this system
settings-security-heading = Security
status-health-heading = Setup health
status-health-caption = Every check the bridge runs on this machine, failures first. Empty when there is nothing to fix.
setup-finalizing-head = Finishing setup…
setup-finalizing-body = Signing you in and preparing { $app }. This only takes a moment.
setup-health-all = All checks
setup-health-all-passed = All checks passed.
setup-health-checked = checked { $ago }
setup-health-diagnostic = gateway diagnostic
setup-health-failures-only = Failures only
setup-health-label-attention = attention
setup-health-label-failing = failing
setup-health-label-healthy = healthy
setup-health-malformed-plugins = malformed plugins
setup-health-never = not checked yet
setup-health-provider-unconfigured = not configured
setup-health-ran-failed = Check finished — some checks did not pass.
setup-health-ran-ok = All checks passed.
setup-health-run = Re-check
toast-gateway-saved = Gateway saved.

# Verdicts ---------------------------------------------------------------------
# One key per code the bridge can emit; the enum is the producer and
# scripts/lint-bridge-i18n.sh checks the two against each other.
tone-section-ok = healthy
tone-section-warn = attention
tone-section-err = down
tone-section-probing = checking…
tone-section-unknown = unknown

gateway-state-unknown = not checked yet
gateway-state-probing = checking…
gateway-state-reachable = reachable · { $latency }ms
gateway-state-unreachable = unreachable · { $reason }

identity-gateway-unreachable = gateway unreachable
identity-verifying = verifying credentials
identity-signed-in = signed in
identity-token-rejected = token rejected by the gateway
identity-signed-out = not signed in

agents-status-cloud-gateway-unreachable = cloud unreachable
agents-status-cloud-verifying = verifying credentials
agents-status-cloud-token-rejected = token rejected — sign in again
agents-status-proxy-unknown = checking…
agents-status-token-valid = Session valid · expires in { $ttl }

overall-syncing = syncing
overall-offline = offline
overall-synced = synced
overall-ready = ready
overall-needs-sign-in = needs sign-in

proxy-state-unknown = checking…
proxy-state-unconfigured = awaiting first host-app probe
proxy-state-listening = listening
proxy-state-refused = connection refused
proxy-state-timeout = timed out
proxy-state-http-error = http error

mcp-auth-unknown = not checked yet
mcp-auth-no-servers = no servers registered
mcp-auth-authenticated = authenticated
mcp-auth-loopback-mismatch = bad loopback secret (403)
mcp-auth-gateway-unauthorized = gateway unauthorized (401)
mcp-auth-not-registered = not in proxy registry (404)
mcp-auth-upstream-error = upstream error
mcp-auth-proxy-unreachable = proxy unreachable
mcp-auth-probe-timeout = did not answer in time
mcp-auth-local-error = could not be checked
mcp-auth-protocol-error = protocol error
mcp-recheck = Re-check
mcp-checking = Checking…
mcp-tools = { $count ->
    [one] { $count } tool
   *[other] { $count } tools
  }
mcp-no-tools = Authenticated — no tools exposed.
mcp-signed-in-as = signed in as { $email }
mcp-checked = checked
mcp-live-roundtrip = Live initialize + tools/list through the loopback proxy.
mcp-open-marketplace = Open in Marketplace
mcp-proxy-url = Proxy URL
mcp-upstream-url = Upstream URL

setup-health-label-not-checked = not checked yet
host-profile-absent = no configuration profile
agent-kind-cli-tool = Command line
agent-kind-desktop-app = Desktop app
settings-schedule-not-installed = Manual — sync from the Marketplace pane

update-phase-downloading = Downloading { $percent }%
update-phase-installing = Installing…
update-phase-failed = { $message }
mcp-rechecked = MCP servers re-checked.
mcp-tools-unavailable = Tools are listed once the server authenticates.
marketplace-detail-auth = Authentication
