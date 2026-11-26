# 12. Risks and open decisions

## 1. Not verified

- Nothing in this plan has been built or run.
- `systemprompt-harness` has never been launched on Windows or signed in to an instance.
- `gpui-kit` 0.9.1 on Linux (Wayland, X11), and type-erased views contributed across a crate boundary, are assumed.
- That `systemprompt-wire`'s codecs cover the client direction `harness-llm` needs.
- The `future` contract was read at `d17b79b70` and is still changing.
- Line counts: harness `0839a31`, bridge `d17b79b70`. Some per-module bridge figures in `architecture/05-bridge-split.md` are marked approximate.
- The behaviour differences between the harness's `sp-*` crates and the bridge are listed as examples to check, not as findings.

## 2. Risks

| # | Risk | Why it is hard | Mitigation |
|---|---|---|---|
| 1 | The `future` contract is not finished | Later stages of that programme remain; amendments run past 185 | S0 to S2 need only the host variant from core. The neutral split (S3) is agreed with that programme's owner first |
| 2 | Splitting the bridge | 75k lines in production; Windows ACL handling is unforgiving | Split in place; verbatim moves; fixtures first; a bridge release on the carved crates as proof; separate security reviews |
| 3 | Splitting `systemprompt-extension` | 30 crates depend on it | Feature split; fallback into `provider-contracts`; measure the neutral closure |
| 4 | Core grows | About 41k lines from the harness and about 60k from the non-GUI bridge become core crates; core publishes 41 crates already and hits rate limits | Expand existing crates first; fix the crate list in S5 with the maintainers; publish only on release; ask whether per-crate versioning from `future` applies |
| 5 | Device code in a server-shaped layer model | Operating-system code and a SQLite store enter a workspace whose rules assume Postgres and HTTP | Path-scoped gates; closure gate keeps the two closures apart; `platform` confinement |
| 6 | Three-system checks in core | Runners are slow and the artefact quota is full | Scoped: only when a device crate changes; no artefacts from core |
| 7 | Contributed views | The toolkit's views are not `dyn`-friendly | Prototype in S3 before views are converted |
| 8 | Toolkit pinned exactly | A bump breaks every fork's views; five advisories arrive through it | Views name toolkit types only through `shared`; advisories live in the template's `deny.toml`; bumps on a minor |
| 9 | Linux desktop | Unverified for the pin | Headless Linux is committed; desktop is gated in S6 |
| 10 | Rewriting the bridge window | 12.4k lines of Rust and a 35k-line web tree are replaced | Fixture states are the parity checklist; views land after the functions they show are proven |
| 11 | The elevated policy writer | An administrator-owned copy of the binary runs as SYSTEM | Verbatim in S5; own review; real Windows machine before any handover |
| 12 | Fleet handover | Host profiles and tasks point at the bridge binary by path; MDM-pinned trust | Keep the on-disk layout; upgrade test; pilot; failure leaves the old binary in place; MDM swap for managed fleets |
| 13 | A background process users did not expect | Other agents lose their proxy if it stops | `tray` slice; clear statement in the window; `serve` for machines with no desktop |
| 14 | Opening host identity | A closed enum on an authorization path gains declared members | Built-ins stay closed; fail closed on both sides; typed `HostId`; security review |
| 15 | A careless or hostile fork | Trusted Rust in a process holding a session | Standing is a correctness boundary, not a sandbox, and is described as such. The instance stays the enforcement point; composition digest in device evidence |
| 16 | The fail-open verdict | The one deliberate exception lives in the reserved governor | Layers have no path to `Allow` on error or timeout; property test |
| 17 | Slice combinations | Eleven slices; a slice that compiles only in the full build is easy to write | Every form is a named composition built and booted in CI |
| 18 | Lockdown claims | Easy to promise more than a user-space process can enforce | State the limit in the product and the external guide |
| 19 | Signing for forks | A fork's installer is a different publisher to the user | Each fork signs with its own identity; the instance advertises download and publisher |
| 20 | Relicensing the shell | BUSL-1.1 code becomes MIT | Sound only once the trust core has left it; a maintainer decision; recorded in the template's changelog |
| 21 | Doing this while the app is unproven | 81k lines never launched | S1 and S2 prove it on Windows and macOS before it is restructured |
| 22 | Parity pressure | Users of the overlay and the bridge need parity before extensibility | Stages 0, 1 and 6 deliver for them; the composition work is invisible to them and can be paced |
| 23 | Two programmes on one branch | The `future` programme owns `future` | This programme's lanes use their own branches and merge into `future` only at agreed points |

## 3. Decisions taken

See `README.md`: D1 to D9.

Consequences already applied in this plan:

- No separate kernel repository; no harness release line; no fifth release lane.
- Proxy, host integrations, machine, lockdown, engine, governor, Drive and update are core, not the extension.
- Forks of the harness stay host `harness`; other coding agents become extension hosts declared by the instance.
- The harness repository's forked gates are deleted, not maintained.

## 4. Decisions still open

| # | Decision | Recommendation | Needed by |
|---|---|---|---|
| O1 | Neutral contract: feature split of `systemprompt-extension`, or move into `provider-contracts` | Feature split | S3 |
| O2 | The device crate list and its place in the five layers | `architecture/02-target-architecture.md` section 3 as the upper bound; fewer crates where an existing one owns the concern | S4 |
| O3 | Does `HostKind::Harness` go to `next` now or wait for `future` | `next` now | S0 |
| O4 | On-disk layout: keep the bridge's, or the harness repository's with an import | Keep the bridge's | S5 |
| O5 | Does the agents-only build keep the name and binary `systemprompt-bridge` | Keep the product name as a brand declaration; one codebase | S5 |
| O6 | Tool-verdict budget and fail-open: harness says 3 s, fail open | Confirm against the bridge's behaviour in the S5 diff | S5 |
| O7 | Extension hosts declared by the instance | Yes, fail closed on both sides | S5 |
| O8 | `lockdown` as a core slice with the stated limit | Yes | S5 |
| O9 | `harness.members`: instance control over device members | Yes | S3, enforced in S8 |
| O10 | Linux desktop: committed or best effort | Headless committed; desktop gated | S6 |
| O11 | Signing identities; where the certificates live once `release-sign.yml` leaves core | Template for the reference build; each install its own | S6 |
| O12 | Length of the bridge deprecation window | Two core minors | S7 |
| O13 | Per-crate versioning for device crates | Follow whatever the `future` programme decides for core | S7 |
| O14 | Whether the wire and module name `bridge` is ever renamed | No; not worth a break | Never, unless asked |
