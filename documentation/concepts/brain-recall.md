# Brain recall

How the brain turns one user turn into a `<team_memory>` block: the components on the path, the data flow of one new-turn recall with the rule that holds at each step, what is measured and where the known limits are. The reference for every declaration, surface and knob is [brain.md](../brain.md); this page is the shape of the read side.

## What the brain is, and is not

The brain is revisioned organisational memory. Facts are captured from documents, comments and entered statements, confirmed by people, scoped to who may see them, and recalled into gateway requests. The read path is deterministic and governed: the same facts, declarations, principal and message produce the same block, every fact is checked on its own before it reaches a request, and every injection is written to a receipt. It runs on Postgres alone. No model is called to decide what a request remembers.

It is not a vector store with a prompt in front of it. Semantic ranking, learned reordering, consolidation and session capture are *seams*, compile-time registrations an installation fills, and core ships no default for any of them. Core is the substrate: declarations, indexes, tiers, fusion, visibility and the fill, each a rule a reader can audit from the receipt back to the knob that declared it.

## Components

All of this lives in `crates/domain/brain`; the seam traits in `crates/shared/extension/src/brain/`; the declaration types in `crates/shared/models/src/brain/declarations/`.

| Component | Modules | Holds |
|---|---|---|
| Declarations and the config plane | `plane/` (`files`, `write`, `reindex`, `hash`), `recall/declared.rs` | `services/brain/` parses into one `DeclaredBrain`, is validated as a set and projected into the plane tables. `DeclaredRecall` is the typed view a recall reads (kinds with their ask words, topics, workflow profiles, `importance.yaml`, `retrieval.yaml`, the synonym groups), cached for a short interval. An apply that changes a topic rebuilds the working set; one that changes a kind rebuilds the links, bridges and weighted vectors; one that changes the synonyms rebuilds the vectors |
| The write side and its derived indexes | `index/{batch,entities,aliases,release}` | Every fact write reindexes its set on the same transaction, eleven statements for a set of any size: `brain_working_set` (one row per fact per scope key it is reachable through, with its stored importance), `brain_fact_links` (the subjects a fact mentions), `brain_subject_links` (the bridges a fact declares between its subject and its entity fields' subjects, one row each way), the entity subjects an entity field with `display` names, `brain_aliases`, and `brain_facts.search_weighted`. A declaration change or an upgrade queues keys only; the consumer sweep drains the `brain_fact_index` projection in sets |
| The recall path | `recall/{new_turn, held, tiers/{matched,subject,linked,working_set,fulltext}, search_plan, fusion, introduce, admit, visible, signal, rerank, budget, candidate, limits}` | One new-turn recall, the data flow below. `recall/service.rs` is the entry; `recall/reuse.rs` the same-turn path that reuses a stored block byte for byte |
| Receipts | `recall/store_receipt.rs`, table `brain_recalls` | One row per recall that reached the brain: the injected `{id, revision}` list, the withheld counts by reason, the tokens, the rendered block, and `reranked`. `ai_requests.memory_recall` carries the one-line descriptor; the response carries the ids in `x-systemprompt-memory-ids` |
| Seams | `crates/shared/extension/src/brain/{retrieval,rerank,recall,source_access,traits}` | `FactRetriever` (adds candidates), `Reranker` (reorders the visible facts), `InjectionPolicy` (chooses the fill), `ImportanceModel`, `SourceAccess`, the audience resolvers, `OutcomeSignal`. Each is discovered through `inventory` and wired by the runtime's `RecallSeams` |

## One new-turn recall, end to end

Every step reads the declarations the plane holds; the knob named at a step is the one that declares its rule. The invariants are set in bold.

1. **Context.** The principal's user, client, actor and `project` attribution select a workflow profile. `now` is read once and is the only clock the tiers, the lifecycle count and the importance score use; `as_of` is `now` unless the request asked for an earlier view. The **held keys** are built once (`recall/held.rs`): `org`, `user:<id>`, `project:<attribution>`, and every audience subject the registered resolvers can enumerate for the user. **Same inputs, same block**: nothing on the path reads a second clock or a random source, and ties break on fact id.

2. **The subjects the message names** (`tiers/matched.rs`). An alias or subject id equal to a word n-gram of the last user message scores `1.0`, served by the btree on `alias_norm` and on `lower(subject_id)` (migration `007_subject_lookup`); an alias trigram-similar to the message is reached through the message's probe words, the tokens no fact carries, and scores its similarity. `alias_min_sim` is the floor, `max_subjects` the cut, `alias_longest_wins` the rule that a project id does not name the client inside it. The matched set is resolved once and shared by the subject and linked tiers.

3. **The five core tiers**, concurrently, each returning ids. **Every tier drives from its own index** and admits a fact through a probe of `brain_working_set` by fact id (migration `006_recall_probe`); no tier materialises the principal's candidate set. *Pinned* (`idx_brain_facts_pinned`). *Subject* (`idx_brain_facts_subject`): the facts about a matched subject, asked kinds first, `subject_limit` rows. *Linked* (`tiers/linked.rs`, `idx_brain_fact_links_subject`): the facts linked to a matched subject at `score × links_weight`; at `links_depth: 2` the facts about each bridged subject (`brain_subject_links`) at a further `× links_decay` and the facts linked to a bridged subject at a further `× links_weight`. **Depth is bounded by declaration**: no link is followed from a linked fact and no bridge from a bridged subject. **The anchor's own facts are the subject tier's**: a fact about the matched subject is never a linked hit, whichever bridge leads back to it, so it takes one rank, not two. **A hidden subject bridges nothing** under `links_require_readable_anchor`: a matched subject with no fact the reader can read under the tier predicate is not walked from. *Context* (`brain_working_set_scope_idx`): the request project's facts by stored importance, asked kinds first, stopping at the limit. *Full text* (`search_plan.rs`, the `search_weighted` GIN): the planner expands each word to its synonym group, drops `search_stop_words` and the groups no fact carries, lets a group above `search_max_df` only rank, requires a `search_quorum` share of the informative groups rarest first (over `search_quorum_over`), requires a group at or under `search_rare_alone_share` alone, plans nothing when the filter could match more than `search_max_ndoc` rows, and runs a second pass at `search_retry_quorum` when the first returns nothing. The lifecycle count (step 5) runs concurrently with the tiers.

4. **Fusion and introduction** (`fusion.rs`, `introduce.rs`). The tiers fuse as ids by weighted reciprocal rank, `Σ weight_tier / (rrf_k + rank)`, ties by fact id, and every id a tier returned is loaded once. Each `FactRetriever` is fused as the `extension` tier: an id a core tier returned takes the retriever's rank beside its own; an id none returned is introduced, round-robin across retrievers, capped at `extension_max_candidates`, and admitted by the tier SQL's own rule, so it reaches steps 5 and 6 as any hit does. A zero `tiers.extension` weight disables introduction. **The extension tier is no signal**: nothing a retriever found opens the context, names a subject or counts as a full-text hit.

5. **Lifecycle.** The tier SQL admitted only facts observed by `as_of`, valid then, not due for review then, and in a recalled status. The reachable facts it kept out are counted in one grouped query and reported as `stale`, `review_due` and `disputed`.

6. **Visibility** (`visible.rs`). `(scope allows or topic audience contains) and granularity allows and source allows`, per fact, failing closed. Role and dimension audiences are answered from the held keys; each distinct membership and source question is asked once per recall (`visible_memo.rs`), and a source check past `recall.source_timeout_ms` withholds the fact as `source`.

7. **The signal** (`signal.rs`), read from the visible facts. The recall *names a subject* when the subject tier returned a visible fact; it *has a full-text hit* when that tier returned a visible one; it *asks a kind* when the message carries one of a declared kind's ask words (`asks`, or the kind id's words). **A hidden subject names nothing and bridges nothing**: a subject whose every fact is hidden from the reader is no signal, and only the subject tier names one, so a fact the linked tier reached through a hidden subject's bridges is injected when visible but opens nothing. Before fusion, under `unknown_name_withholds_full_text`, a message that matched no subject and carries a capitalised word no fact carries has the full-text tier emptied (`unknown_name`) and an asked kind is then no signal.

8. **Reranker** (`rerank.rs`). The first registered `Reranker` sees every visible fact with its fused score, tiers, kind and statement, and returns an ordering of a subset of the ids, under `reranker_timeout_ms`. Returned ids first in their order, omitted ids after in fused order, unknown ids ignored, an empty result or a failure leaves the fused order. **A reranker sees only what the reader may see**, because it runs after step 6.

9. **The fill** (`budget.rs`). `usage_weight × (cited_rate − ignored_rate)` is added to each fused score; a fact under `min_fused_score` is `low_score`. Facts are taken in `fill_order` (or the reranker's order) while under their kind's quota, `max_facts` and the token budget. When a subject is named, `linked_share` is held for linked hits and context-only facts may take `context_share`; past it, `context_cap`. **The gate**: with `context_requires_signal` and no signal (no named subject, no asked kind, and no full-text hit or `full_text_opens_context: false`), every context-only fact is `context_cap` and every extension-only fact is `no_signal`; a fact a second core tier also found is never touched.

10. **The block and the receipt.** The facts are rendered as `<team_memory scope=… as_of=…>`, appended after the client's system prompt, and the `brain_recalls` row is written with the injected refs, every withheld count, the tokens, the rendered bytes and `reranked`. A recall past `recall.deadline_ms` injects nothing into this request, finishes in the background and stores its block for the turn's next request.

## What is measured, and how

The measurement rig lives in the Astound installation (`deploy/loadtest/brain`): a generated corpus of clients, projects, people and their facts at three scales (3k, 30k and 300k facts), eight personas with different held keys, a golden set of prompts per recall class, and `eval.py`, which runs every prompt through the admin simulator and, for a sample, through the gateway itself, and stores each run as a row of `brain_eval_runs` (the Evals tab of `/admin/brain`) with the declared hash it ran under. The bars are the memory report's §6.2 set: recall by class (named subject, paraphrase, temporal, cross-entity), precision at three, negative cleanliness (a prompt about something the persona may not see injects only org or pinned facts), leaks (zero, always), simulator-to-gateway parity, deadline rate, the latency the recall adds at p50 and p95, determinism across repeated runs, and capture throughput.

Where the recall path stands on core `03abde55c` with the Astound declaration (runs 46, 50 and 49 of `brain_eval_runs`):

| bar | target | 3k (run 46) | 30k (run 50) | 300k (run 49) |
|---|---|---|---|---|
| recall, named subject | ≥ 0.95 | 1.00 | 1.00 | 0.98 |
| recall, paraphrase | ≥ 0.80 | 0.94 | 0.64 | 0.44 |
| recall, temporal | ≥ 0.90 | 1.00 | 0.80 | 0.455 |
| recall, cross-entity | ≥ 0.80 | 0.99 | 0.90 | 0.82 |
| precision at 3 | ≥ 0.80 | 0.80 | 0.79 | 0.82 |
| negative clean | ≥ 0.90 | 0.87 | 0.93 | 0.96 |
| leaks | 0 | 0 | 0 | 0 |
| gateway parity | 1.0 | 1.00 | 1.00 | 0.98 |
| deadline rate | < 1 % | 0 | 0 | 0 |
| latency added, p50 ms | ≤ 20 | 23 | 35 | 103 |
| determinism | 100 % | 1.00 | 1.00 | 0.99 |

The open bars and their causes: 3k negatives at 0.87, where the four remaining cases inject readable facts that carry the asked name (a project named after a hidden client), a golden-model question rather than a rule; 30k paraphrase at 0.64, template twins and fuzzy aliases the lexical tiers cannot bridge without a real embedding; 30k temporal at 0.80, where a person's other-project team facts share the linked list with the project's own and reciprocal-rank fusion flattens the depth-2 decay; 300k p50 at 103 ms, the tier cost at that scale with the dense tier on; 300k temporal at 0.455, where the rig's dispute volume withholds the contact revisions the temporal cases expect, a rig question before a rule.

## Known limits

Each is registered on the programme's status page with its cause and fix; one line each here.

- **Depth 2 is a bounded two-hop declared join**, not graph traversal: fixed depth, entity-reference edges only, no typed or temporal edges, no path ranking, and reciprocal-rank fusion flattens the per-hop decay (a bridged hit at linked rank 1 weighs like a direct one). Depth-2 hits as their own tier weight is the design item.
- **The dense tier costs latency and breaks parity.** With the hashing embedder it adds about 12 ms at p50 and the simulator and gateway intermittently disagree on order; the provider embedder is unmeasured. The in-round-trip load of its unseen ids, or off by default, is the decision.
- **Capture is CPU-bound in-process.** The consumer drains at 150 to 170 events per second at 3k against a 500 bar, and the rate falls as the corpus grows; the round cost is super-linear in both the batch and the table.
- **A kind declaration change reprojects the corpus inside the apply.** Adding a field to a kind rewrites every fact's weighted vector in one transaction; at 250k facts that is tens of minutes during which the declaration is invisible to recall. The fix is the keys-only queue the sweep drains, as migration 008 already does.
- **Ask words are lists.** A kind is asked only when a message token equals a hand-listed word; measured offline, nearest-kind similarity under the hashing embedder cannot replace the lists at any floor, only supplement them.
- **The explain mirror is a copy.** The rig's `explain.py` duplicates the tier SQL in Python and must be re-copied each wave; an `admin brain explain` over the stored receipt replaces it.
- **Extension-owned schema sits outside the migration contract.** The dense tier creates its embeddings table at runtime; core needs a convention for an extension's declared DDL with a ladder rung.

## See also

- [brain.md](../brain.md) — the reference: declarations, every `retrieval.yaml` knob at its default, the profile, HTTP API, MCP tools, headers, jobs and seams.
- [architecture.md](architecture.md) — the crate layers the brain domain sits in, and how a request reaches it.
- [extensions.md](extensions.md) — the compile-time extension model the seams are built on.
