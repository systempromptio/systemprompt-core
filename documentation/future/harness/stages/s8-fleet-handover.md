# S8. Fleet handover and bridge removal

**Goal.** Installed bridges become the template-built binary without their users doing more than restarting; then the bridge build leaves core.

**Entry.** S6 and S7 closed.

**Specification.** `../architecture/10-release-and-fleet-handover.md`, `../architecture/05-bridge-split.md` sections 3 and 4.

This stage has little parallel implementation and many decisions that belong to people. It is short in lanes and long in elapsed time.

## Wave A — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S8-A1 Successor start-up** | core | First start as a successor: wait for the predecessor; import the bridge's data root into the single device layout once (config, policy store, loopback secret byte for byte, ledgers, sentinel), verify the sealed credential and loopback secret are readable; re-register scheduler entries and the policy-writer task; rewrite binary paths in host profiles; run the self-check; only then remove the old binary. Any failure leaves the old binary in place and reports through the heartbeat |
| **S8-A2 Upgrade test** | template | A workflow job: install the last bridge release, configure hosts against the scripted instance, upgrade, assert every item in the handover table and that each host still reaches the proxy with its existing token |
| **S8-A3 Command parity** | template | Every bridge command, flag and exit code answered by the bridge composition, proven by the conformance fixtures |

Then **S8-A4 (review)** of A1: the import of secrets and the profile rewrite.

**Stage-internal L2.** The upgrade test green on Windows and macOS.

## Wave B — people

In order, each a decision reserved to people:

1. The manual walk on real machines, including a standard user with an administrator-pinned key on Windows.
2. A pilot on maintainers' devices.
3. The handover release: a final bridge release whose update target is the bridge composition.
4. One consenting install's fleet.
5. Remaining fleets; managed fleets by a package swap in their device management.
6. Each fleet's owner confirms it has moved. There is no fixed window: the bridge build stays in core until every known fleet has confirmed.

## Wave C — removal, three lanes, parallel

Opens only when wave B is complete.

| Lane | Repository | Work |
|---|---|---|
| **S8-C1 Delete the bridge** | core | The bridge directory and lockfile; the bridge-only gates, recipes and workflow jobs; the link-time registration exception |
| **S8-C2 Scope the native check** | core | Replace the unconditional native matrix with the scoped device job from S1 |
| **S8-C3 Documents and channels** | core, template | Core's guides and current-functionality documentation; the template's bridge directory; package-channel manifests repointed |

## Stage gate (L2)

- Core's push run has no unconditional desktop job and is green.
- The template's release produces every artefact the bridge's release did.

## Exit criteria

1. No supported installation runs the bridge binary.
2. Core's release builds no binary.
3. The minimum client version on each instance has been raised by its operator.
4. This directory's content that describes shipped behaviour moves out of `documentation/future/` into the current documentation.

## Decisions reserved to people

Everything in wave B; raising the minimum client version; deleting the bridge.
