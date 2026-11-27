# 12. Risks and open decisions

## 1. Not verified

- Nothing in this plan has been built or run.
- `systemprompt-harness` has never been launched on Windows or signed in to an instance.
- `gpui-kit` 0.9.1 on Linux (Wayland, X11), and type-erased views contributed across a crate boundary, are assumed.
- That core's wire codecs cover the client direction `harness-llm` needs.
- The `future` contract was read at `d17b79b70` and is still changing.
- Line counts: the harness at the lift pin (D45; figures in `architecture/06-harness-repo-disposition.md`), bridge `d17b79b70`. Some per-module bridge figures in `architecture/05-bridge-split.md` are marked approximate.
- Every S0 lane wrote by reading. What each lane compiled or ran, and what it did not, is in its hand-off note; the S0 stage gate is the first run of the bridge build, the bridge test crates and the two new gates' self-tests.
- The reference-test catalogue is a first classification from test declarations: 64 rows carry a doubt, and its case counts are a lower bound (`testing/04-test-migration.md`).
- The behaviour differences between the harness's `sp-*` crates and the bridge are listed as examples to check, not as findings.

## 2. Risks

| # | Risk | Why it is hard | Mitigation |
|---|---|---|---|
| 1 | The `future` contract is not finished | Later stages of that programme remain; amendments run past 185 | S0 to S2 need only the host variant and `host_extensions` from core. The machinery move (S5) is timed with that programme's owner |
| 2 | Splitting the bridge | 75k lines in production; Windows ACL handling is unforgiving | Split in place; verbatim moves; fixtures first; a bridge release on the carved crates as proof; separate security reviews |
| 3 | Moving the composition machinery | 44 crates depend on `systemprompt-extension` | Decided: the machinery moves into `provider-contracts` and `extension` re-exports it, so dependants do not change; one early L2 run in S5 |
| 4 | Core grows | About 41k lines from the harness and about 60k from the non-GUI bridge become core crates; core publishes 41 crates already and hits rate limits | Eight crates, decided; everything else merges into existing crates; publish only on release |
| 5 | Device code in a server-shaped layer model | Operating-system code and a SQLite store enter a workspace whose rules assume Postgres and HTTP. The lift's SQLite binding and the server's optional one must resolve to one native library version | Schema, Postgres and HTTP gates path-scoped with a written reason each, coding-standard gates met (`architecture/09-quality-and-gates.md` section 1); closure gate keeps the two closures apart; `platform` confinement |
| 6 | Three-system checks in core | Windows and macOS runners are slow | Scoped: only when a device crate changes; runs store nothing |
| 7 | Contributed views | The toolkit's views are not `dyn`-friendly | Prototype in S5 before views are converted |
| 8 | Toolkit pinned exactly | A bump breaks every fork's views; five advisories arrive through it | Views name toolkit types only through `shared`; advisories live in the template's `deny.toml`; bumps on a minor |
| 9 | Linux desktop is required and unverified | The pinned window toolkit may not support Wayland and X11; raising or patching the pin can break every fork's views | Verify first in S7; if it fails, the pin change is scheduled work with every view re-checked on the tour on three systems |
| 10 | Rewriting the bridge window | 12.4k lines of Rust and a 35k-line web tree are replaced | Fixture states are the parity checklist; views land after the functions they show are proven |
| 11 | The elevated policy writer | An administrator-owned copy of the binary runs as SYSTEM | Verbatim in S6; own review; real Windows machine before any handover |
| 12 | Fleet handover and the one-time import | Host profiles and tasks point at the bridge binary by path; MDM-pinned trust | Import once into the single layout, loopback secret byte for byte; upgrade test; pilot; failure leaves the old binary in place; MDM swap for managed fleets |
| 13 | A background process users did not expect | Other agents lose their proxy if it stops | `tray` slice; clear statement in the window; `serve` for machines with no desktop |
| 14 | Opening host identity | A closed enum on an authorization path gains declared members. Manifest fields are keyed by the closed enum, so a reader fails the whole parse on a host it does not know | Built-ins stay closed; fail closed on both sides; typed `HostId`; a host-owned host is opt-in and raises the manifest's minimum bridge version (D42); security review |
| 15 | A careless or hostile fork, or a modified client | In the local modes the runtime is on the user's machine and could skip the verdict | The instance always controls model calls; `Sandboxed` bounds the agent by the operating system; an instance can allow `Remote` only; the composition digest lets an instance recognise a build. Standing is a correctness boundary, not a sandbox |
| 16 | The fail-open verdict | The one deliberate exception lives in the reserved governor | Layers have no path to `Allow` on error or timeout; property test |
| 17 | Slice combinations | Eleven slices; a slice that compiles only in the full build is easy to write | Every form is a named composition built and booted in CI |
| 18 | Lockdown claims | Easy to promise more than a user-space process can enforce | State the limit in the product and the external guide |
| 19 | Signing for forks | A fork's installer is a different publisher to the user | Each fork signs with its own identity; the instance advertises download and publisher |
| 20 | Relicensing the shell | BUSL-1.1 code becomes MIT | Only the UI, desktop and brand crates are relicensed, as they enter the template; governed code never sits there |
| 21 | Doing this while the app is unproven | The lifted code has never been launched on Windows or signed in to an instance | S1 and S2 prove it on Windows and macOS before it is restructured |
| 22 | Parity pressure | Users of the overlay and the bridge need parity before extensibility | S1, S2 and S8 deliver for them; the composition work is invisible to them and can be paced |
| 24 | Sandbox strength differs by platform | The policy is one, the enforcers are not equal; Windows has none by decision | A conformance suite per backend; the whole runtime is confined; `Sandboxed` is unavailable with its reason where no backend exists; Windows users who need confinement use `Remote` |
| 25 | Remote sessions are a hosted execution service | The instance runs arbitrary commands for its users: isolation, egress, resource abuse and cost. Whether the chosen hosting can start each isolation level is unverified | A typed isolation level with the sandbox as floor; a prototype in the actual hosting before the stage; the instance's own egress allowlist; per-user and per-node limits; credentials scoped to one session; an independent review |
| 26 | A local machine that is offline, or untruthful | Its session cannot publish or receive control; and what it publishes is its own account | A local journal resumes the upload; other clients show the session as offline; control waits or expires; device-reported records are marked as such |
| 28 | Sandbox friction | A strict policy breaks package installs, language servers and similar | The prototype measures what breaks; presets; a per-session widening request answered like an approval |
| 29 | Repository checkout for remote sessions | Needs per-user credentials for source hosts on the instance; what the existing connectors offer is unverified | Established by the S4 prototype before the contract is written |
| 27 | `Unrestricted` is exactly that | Full access to the user's machine | The mode is typed, recorded and shown on every client; an instance or an administrator can forbid it; permission rules and sensitive-target asks still apply |
| 30 | A web view in the shell | The preview pane brings back a browser engine the harness had excluded | Confined to one origin, no bridge, its own opt-in slice, system-browser fallback, independent review |
| 31 | Unattended execution | A scheduled session runs with no one watching | `Remote` only; per-user caps; approvals deny on timeout; a failing schedule pauses itself; independent review |
| 32 | Memory retains the wrong thing | Something sensitive or wrong is recalled later | The memory capability's own validation and policy; recall is visible per turn; a user can remove an item; an instance can disable the slice |
| 23 | Two programmes on one branch | The `future` programme owns `future` | This programme's lanes use their own branches and merge into `future` only at agreed points |

## 3. Decisions taken

D1 to D9 are in `README.md`. D10 to D41 were settled on 2026-11-26 and are applied throughout this directory:

| # | Decision | Taken |
|---|---|---|
| D10 | Crate list | Eight new core crates: `systemprompt-engine`, `-device`, `-harness`, `-drive`, `-proxy`, `-machine`, `-hosts`, `-runner`. Products are features of `-hosts`; lockdown is a feature of `-machine`. Everything else merges into existing crates |
| D11 | Publishing | All eight are published with core |
| D12 | Shared composition machinery | Moved down into `systemprompt-provider-contracts`; `systemprompt-extension` re-exports it. No cargo feature separates server from device |
| D13 | Host variant | `HostKind::Harness` lands on `next` and `future` |
| D14 | Engine placement | The engine and governance go straight into core in S1; only the shell goes to the template. No interim with governed code in the template, and no separate promotion stage |
| D15 | Engine crates | The eleven harness engine crates merge into `systemprompt-engine` as modules in S1 |
| D16 | On-disk layout | One channel-aware layout per brand. An installed bridge's data is imported once at the handover |
| D17 | Bridge name | The no-window composition keeps the product and binary name `systemprompt-bridge`, as a brand declaration |
| D18 | Other coding agents | Yes: built-in hosts stay a closed enum; further hosts are declared by the instance and refused or withheld when undeclared |
| D19 | Instance control over members | `harness.members` is in the S5 contract; server-side publishing arrives in S9 |
| D20 | Lockdown | A feature of `systemprompt-machine`, with its limit stated: it governs agents it manages and reports unmanaged ones |
| D21 | Linux | Desktop is a requirement, not best effort. If the pinned window toolkit does not support it, the pin is raised or patched in S7 |
| D22 | Bridge deprecation | No fixed window. The bridge build stays in core until every known fleet's owner confirms it has moved |
| D23 | Reference agent naming | Named in this directory and in workflow configuration only; never in source, test names or strings |
| D24 | History | Plain import recording the source commit; history stays in the archived repository |
| D25 | Evidence | A development process, not continuous integration. An agent session attached to real Windows and macOS machines runs one command per machine at stage closure. Hosted runners build and test on three systems and store nothing |
| D27 | Execution modes | One runtime in core, three typed modes: `Unrestricted` and `Sandboxed` on a desktop, `Remote` in the instance's sandbox. The loop is one implementation; a mode is an argument, not a fork (`architecture/11-execution-modes.md`). Withdrawn: a server-side loop sending each tool call to a device |
| D28 | Sandbox | One policy type, enforced by each platform's own mechanism; the Linux backend serves both a Linux desktop and the instance. `Sandboxed` refuses to start where no backend exists; it never degrades silently |
| D29 | Session record | The instance records every session in every mode. A local runtime publishes its events and receives control. The stream uses the AG-UI vocabulary core already defines |
| D30 | Approvals | One mechanism in every mode: the governance approval table keyed by call id |
| D32 | Surfaces and modes | Phones, the web and scheduled work use `Remote` only. Desktops offer all three, subject to the instance's allowed modes |
| D31 | Shared client layer | State machines (stream handling, approval queue, notification routing), not screen-shaped models. Not shared with the web console as code; the web consumes the same stream |
| D33 | Windows | No native Windows sandbox is built. A Windows desktop offers `Unrestricted` and `Remote`; `Sandboxed` is unavailable there, with its reason |
| D34 | What `Sandboxed` confines | The whole runtime process, not only its tools, so nothing escapes by bypassing the execution environment |
| D35 | Remote isolation | A typed level: the Linux sandbox always; a container per session by default where more than one organisation's users share an instance; a small virtual machine where the hosting offers it |
| D36 | Provenance | A session record reported by a device is marked as such wherever it is stored or shown |
| D37 | Prototypes | The sandbox stage and the remote stage each begin with a throwaway prototype that fixes the backend, default policy and isolation level |
| D38 | Parallel sessions | A session may run in its own linked working copy on its own branch; required for a second writing session on one repository. A working copy with unmerged, unpushed work is never removed silently |
| D39 | Terminal | A user's terminal in the session's workspace, inheriting the session's mode and sandbox |
| D40 | Preview pane | The platform's web view, confined to the session's preview origin with no bridge into the app. The single exception to the shell having no web view; an opt-in slice |
| D41 | Scheduled sessions and memory | Built in S4 and shown in S5, not left to the last stage. Schedules start `Remote` sessions only. Memory uses core's memory capability; the runtime keeps no store of its own |
| D26 | Signing | The reference builds reuse the identities the bridge release uses today; each install signs its own brand |

Settled on 2026-11-27, from what S0 waves A and B found:

| # | Decision | Taken |
|---|---|---|
| D42 | Host `harness` is opt-in per instance | A host-owned host (`config_owner` is `Host`) is withheld unless the instance declares it in `external_agents` and enables it. Enabling one raises the manifest's minimum bridge version, so an older bridge gets the upgrade-required refusal and not a parse error. Host-owned hosts are never in the well-known bridge document. Bridge-owned hosts keep absent-means-enabled (`architecture/04-host-integrations.md` 3.1.2) |
| D43 | A fork's brand | Travels in its own header and its own column, introduced with `BrandDecl` in S5. `x-systemprompt-client` stays the closed client kind; every fork sends `harness` there (`04` 3.1.6) |
| D44 | `host_extensions` | Kept, and specified: additive capability only; absent means off; typed kebab-case ids; declared only by a host-owned host; part of the signed manifest. It is on the programme branch and `future`, not on `next`. Its relation to `harness.members` is fixed in the S5 contract (`04` 3.3) |
| D45 | Lift pin | The lift pins the harness repository at `a214511`. The repository takes no further commits until it is archived |
| D46 | Lift placements | `sp-sync`, `sp-extension`, the Drive's engine half, Brain, notifications and Slack land in `systemprompt-harness`. Engine ids whose names collide with existing ones go in an `identifiers::engine` submodule. Link-time registration through `inventory` is replaced in S1 by an explicit registration list. The owned-task types the lifted callers need are added to `systemprompt-traits`. Tables: `architecture/06-harness-repo-disposition.md`, `stages/s1-lift.md` |
| D47 | Reference pin and catalogue vocabulary | The reference agent is pinned at tag `v2.4.2` (commit `fc31a485`, MIT). `not-applicable` gains two reason codes, `no-feature` and `reference-internal`. Deferred rows stay deferred to the stage that delivers the feature (`testing/04-test-migration.md`) |
| D48 | Scripted instance on the native matrix | The scripted-instance crate does not depend on the test workspace's dependency-unification crate, which pulls the server stack and its database driver. It is listed among that crate's exclusions, so the golden suites that link it run on Windows and macOS (`testing/02-scripted-instance.md` section 2) |
| D49 | The runner and the kernel closure | `systemprompt-runtime` stops depending on `systemprompt-agent-runs`; the one type it takes from there moves to a shared-layer contract. The closure gate gets no exception for the runner (`architecture/02-target-architecture.md` section 3). The capability-gate half is O5 |

Consequences worth restating:

- About 100,000 lines join core's root workspace over S1 and S6, and core gains a scoped check on Windows and macOS. The code is migrated, not new: about 60,000 lines are the bridge's, already in the repository as a separate workspace, and about 41,000 come from the harness repository, of which about 14,000 are deleted in S6. With the bridge's window and web tree retired, the repository ends smaller than it is today.
- D21 makes the window toolkit's Linux support a programme risk, not an option (risk 9).
- D22 means core may carry the bridge build for as long as the slowest fleet takes.
- D25 means a stage cannot close unless both machines are online and linked.
- D42 means no instance serves host `harness` by accident, and that turning it on is a minimum-client-version change, which is a person's decision.
- D45 freezes the source. A change the harness needs before S1 closes is made after the lift, in core or the template.
- D27 keeps local execution as efficient as any local agent and gives parity with one in `Unrestricted`. In the two local modes the runtime runs on a machine its user controls, so tool governance there is as strong as the client is unmodified; an organisation that needs certainty allows `Remote` only.

## 4. Decisions still open

| # | Decision | Recommendation | Needed by |
|---|---|---|---|
| O1 | Tool-verdict budget and fail-open: the harness uses 3 s and fails open | Confirm against the bridge's behaviour in the S6 diff; the bridge's wins unless there is a reason | S6 |
| O2 | Per-crate versioning for device crates | Follow whatever core's `future` programme decides for core | S8 |
| O3 | Whether the wire and module name `bridge` is ever renamed | No; not worth a break | Never, unless asked |
| O4 | App icon artwork | From the owner | S7 |
| O5 | The unit of `systemprompt-runner`. It is a device crate (`capability:device`) that the server's `agent-runs` (`capability:agents`) must link, and `lint-capability-deps` refuses one capability depending on another | Give the runner a unit of its own that the gate names as linkable from both sides, with the rule that it depends on nothing above `shared`. The alternative, moving the crate to the shared layer, changes D10's placement | S9 (S9-A3) |
| O6 | Which principal the harness's session is, and so the attestation tier its traffic records (`architecture/04-host-integrations.md` 3.1.4) | The user's own principal, recorded as `declared`; a stronger tier only with device evidence the instance can verify (the composition digest, S9-A2) | S2 (S2-A1) |
| O7 | The per-marketplace `harness` settings block the lifted manifest reader expects; core's manifest has none | Read as absent in S1 (defaults); S9-A1 publishes host settings | S1 (S1-B4) |
