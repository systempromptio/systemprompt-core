# 12. Risks and open decisions

## 1. Not verified

- Nothing in this plan has been built or run.
- `systemprompt-harness` has never been launched on Windows or signed in to an instance.
- `gpui-kit` 0.9.1 on Linux (Wayland, X11), and type-erased views contributed across a crate boundary, are assumed.
- That core's wire codecs cover the client direction `harness-llm` needs.
- The `future` contract was read at `d17b79b70` and is still changing.
- Line counts: harness `0839a31`, bridge `d17b79b70`. Some per-module bridge figures in `architecture/05-bridge-split.md` are marked approximate.
- The behaviour differences between the harness's `sp-*` crates and the bridge are listed as examples to check, not as findings.

## 2. Risks

| # | Risk | Why it is hard | Mitigation |
|---|---|---|---|
| 1 | The `future` contract is not finished | Later stages of that programme remain; amendments run past 185 | S0 to S2 need only the host variant from core. The machinery move (S4) is timed with that programme's owner |
| 2 | Splitting the bridge | 75k lines in production; Windows ACL handling is unforgiving | Split in place; verbatim moves; fixtures first; a bridge release on the carved crates as proof; separate security reviews |
| 3 | Moving the composition machinery | 44 crates depend on `systemprompt-extension` | Decided: the machinery moves into `provider-contracts` and `extension` re-exports it, so dependants do not change; one early L2 run in S4 |
| 4 | Core grows | About 41k lines from the harness and about 60k from the non-GUI bridge become core crates; core publishes 41 crates already and hits rate limits | Eight crates, decided; everything else merges into existing crates; publish only on release |
| 5 | Device code in a server-shaped layer model | Operating-system code and a SQLite store enter a workspace whose rules assume Postgres and HTTP | Path-scoped gates; closure gate keeps the two closures apart; `platform` confinement |
| 6 | Three-system checks in core | Windows and macOS runners are slow | Scoped: only when a device crate changes; runs store nothing |
| 7 | Contributed views | The toolkit's views are not `dyn`-friendly | Prototype in S4 before views are converted |
| 8 | Toolkit pinned exactly | A bump breaks every fork's views; five advisories arrive through it | Views name toolkit types only through `shared`; advisories live in the template's `deny.toml`; bumps on a minor |
| 9 | Linux desktop is required and unverified | The pinned window toolkit may not support Wayland and X11; raising or patching the pin can break every fork's views | Verify first in S6; if it fails, the pin change is scheduled work with every view re-checked on the tour on three systems |
| 10 | Rewriting the bridge window | 12.4k lines of Rust and a 35k-line web tree are replaced | Fixture states are the parity checklist; views land after the functions they show are proven |
| 11 | The elevated policy writer | An administrator-owned copy of the binary runs as SYSTEM | Verbatim in S5; own review; real Windows machine before any handover |
| 12 | Fleet handover and the one-time import | Host profiles and tasks point at the bridge binary by path; MDM-pinned trust | Import once into the single layout, loopback secret byte for byte; upgrade test; pilot; failure leaves the old binary in place; MDM swap for managed fleets |
| 13 | A background process users did not expect | Other agents lose their proxy if it stops | `tray` slice; clear statement in the window; `serve` for machines with no desktop |
| 14 | Opening host identity | A closed enum on an authorization path gains declared members | Built-ins stay closed; fail closed on both sides; typed `HostId`; security review |
| 15 | A careless or hostile fork | Trusted Rust in a process holding a session | With the loop on the server, a modified client can no longer skip the verdict for the native engine: the server decides before it issues a tool call. A fork still controls what its own tool host executes; standing remains a correctness boundary, not a sandbox |
| 16 | The fail-open verdict | The one deliberate exception lives in the reserved governor | Layers have no path to `Allow` on error or timeout; property test |
| 17 | Slice combinations | Eleven slices; a slice that compiles only in the full build is easy to write | Every form is a named composition built and booted in CI |
| 18 | Lockdown claims | Easy to promise more than a user-space process can enforce | State the limit in the product and the external guide |
| 19 | Signing for forks | A fork's installer is a different publisher to the user | Each fork signs with its own identity; the instance advertises download and publisher |
| 20 | Relicensing the shell | BUSL-1.1 code becomes MIT | Only the UI, desktop and brand crates are relicensed, as they enter the template; governed code never sits there |
| 21 | Doing this while the app is unproven | 81k lines never launched | S1 and S2 prove it on Windows and macOS before it is restructured |
| 22 | Parity pressure | Users of the overlay and the bridge need parity before extensibility | S1, S2 and S7 deliver for them; the composition work is invisible to them and can be paced |
| 24 | A server-to-device execution channel | A compromised instance could direct commands at every connected desktop | The device's own rules are a gate the server cannot move; roots checked at time of use; sensitive targets always ask; per-device binding and authentication; three independent reviews in S3 |
| 25 | Server load and state | Every active session holds a loop, a transcript and a lease on the instance | Runner nodes; per-user and per-node limits; one persisted step at a time; retention rules; the added latency per tool call is measured at the S3 gate |
| 26 | A device that goes offline mid-turn | The turn waits on a machine that may not return | A visible waiting state on every client; a notification; expiry returns a failure to the model; cancel from any client |
| 27 | No precedent | No shipped product was found that runs the loop on a server and tools on the user's own machine | S3 comes after the deterministic suite exists; the same scenarios must produce the same traces; an early spike of the request channel before S3 opens |
| 23 | Two programmes on one branch | The `future` programme owns `future` | This programme's lanes use their own branches and merge into `future` only at agreed points |

## 3. Decisions taken

D1 to D9 are in `README.md`. Settled on 2026-11-26, and applied throughout this directory:

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
| D19 | Instance control over members | `harness.members` is in the S4 contract; server-side publishing arrives in S8 |
| D20 | Lockdown | A feature of `systemprompt-machine`, with its limit stated: it governs agents it manages and reports unmanaged ones |
| D21 | Linux | Desktop is a requirement, not best effort. If the pinned window toolkit does not support it, the pin is raised or patched in S6 |
| D22 | Bridge deprecation | No fixed window. The bridge build stays in core until every known fleet's owner confirms it has moved |
| D23 | Reference agent naming | Named in this directory and in workflow configuration only; never in source, test names or strings |
| D24 | History | Plain import recording the source commit; history stays in the archived repository |
| D25 | Evidence | A development process, not continuous integration. An agent session attached to real Windows and macOS machines runs one command per machine at stage closure. Hosted runners build and test on three systems and store nothing |
| D27 | Where native sessions run | On the instance. The loop is server-side; a device executes tool requests behind its own permission rules; every client renders one stream. Third-party agents are unaffected (`architecture/11-server-owned-sessions.md`) |
| D28 | Tool request granularity | A whole tool call per request, not low-level file and terminal operations |
| D29 | Session stream | The AG-UI event vocabulary core already defines, on a durable per-turn log with resume |
| D30 | Waiting on a person | Reuses the governance approval table keyed by call id; waiting on a device and on a person are one state machine |
| D31 | Shared client layer | State machines (stream handling, approval queue, notification routing), not screen-shaped models. Not shared with the web console as code; the web consumes the same stream |
| D26 | Signing | The reference builds reuse the identities the bridge release uses today; each install signs its own brand |

Consequences worth restating:

- About 100,000 lines join core's root workspace over S1 and S5, and core gains a scoped check on Windows and macOS. The code is migrated, not new: about 60,000 lines are the bridge's, already in the repository as a separate workspace, and about 41,000 come from the harness repository, of which about 14,000 are deleted in S5. With the bridge's window and web tree retired, the repository ends smaller than it is today.
- D21 makes the window toolkit's Linux support a programme risk, not an option (risk 9).
- D22 means core may carry the bridge build for as long as the slowest fleet takes.
- D25 means a stage cannot close unless both machines are online and linked.
- D27 makes the instance stateful for every active session and adds a server-to-device execution channel. It is new ground: no shipped product was found that documents this split.

## 4. Decisions still open

| # | Decision | Recommendation | Needed by |
|---|---|---|---|
| O1 | Tool-verdict budget and fail-open: the harness uses 3 s and fails open | Confirm against the bridge's behaviour in the S5 diff; the bridge's wins unless there is a reason | S5 |
| O2 | Per-crate versioning for device crates | Follow whatever core's `future` programme decides for core | S7 |
| O3 | Whether the wire and module name `bridge` is ever renamed | No; not worth a break | Never, unless asked |
| O4 | App icon artwork | From the owner | S6 |
