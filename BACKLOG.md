# DEOS Backlog

> Open framework work only. Specifications own normative behavior; code and tests own implementation and regressions; generated artifacts own exact resource bindings; Experiment Records own decision evidence. `BACKLOG.md` alone owns release scope and remaining work. Remove completed tasks from the active backlog without deleting their evidence.
>
> Pre-`1.0`: no DEOS network launches before `1.0`. The `0.7.x` line is fresh-genesis. Published tags and reviewed history remain immutable. Breaking authoring, storage and API changes must be explicit and tested, never silent reinterpretations of existing Contracts.

## DEOS 0.7.27 — Current-State Actors, Persistent Live Ring and Certified Parking

**Planning status:** repository intake was reconciled on branch `0.7.27` from baseline commit `5a99deee8ba5d0b3046f661eeddbd8f32b2441e5`, whose sole change above published `v0.7.26` commit `c16675edf03e3af0d6dcf3080a31c64b49765a5b` is this consolidated backlog. The current specification and runtime still implement the historical Trigger latch, `PercentageOfLastFunding`, per-Step ticket publication and strict FIFO service; no new-model implementation or measurement is imported as completed work. N0.1 is closed, all 35 remaining workstreams are pending, all previous N-task IDs are retained, and N3.6, N3.7, N3.8 and N6.5 are the four added IDs.

**Release outcome:** deliver autonomous current-state Actors with multi-block programs, live sovereign-balance amounts and bounded retry/error policies. Keep useful near-term continuations resident in a live scheduling ring instead of removing and republishing a successor after every Step. Keep other nonterminal Actors outside ordinary service only under an explicit wake or timed-review contract. Implement generation-safe asynchronous reclamation and sound physical/resource coverage, and demonstrate a materially useful end-to-end result without hiding negative-check, indexing or cleanup costs.

**The selected direction is a logical service design, not a preselected linked-list implementation.** A compact intrusive carrier, paged carrier or adequate existing representation may realize it. Choose one using the smallest necessary physical comparison. “Ring” must not become an excuse to reintroduce a second executor, unbounded pointer traversal, lost wakeups or an undocumented ordering policy.

**Release boundary:** `0.7.27` closes one complete, measured semantic and physical design. A later `0.7.28` campaign may tune its encodings and residual costs. Correctness, actual index representation and production Weight coverage cannot be deferred; exhaustive bitmap/page/fanout/cache sweeps are not required now.

### What changed from the preceding plan

The current-state mandate and historical-evidence requalification remain. The service architecture is now explicit:

```text
                bounded dependency notification / timed review
                                   |
                                   v
                         ACTIVATION CHECK
                        /                \
         false + safe wait plan          current start condition true
                    |                              |
                 PARKED                            v
                    ^                       PERSISTENT LIVE RING
                    |                         |              |
                    +---- no current work ----+              |
                                             next Step       | known later eligibility
                                             stays resident  v
                                                        SLEEP INDEX
                                                             |
                                                        due -> live

Owner controls / terminal outcome -> DISABLED or RETIRED
Retired old generation -> bounded RECLAIMING -> reclaimed state
Sovereign custody is not moved or deleted by these scheduler transitions.
```

The preceding conversational sketch is not a completed algorithm. This plan expressly closes its missing proof obligations: a saved ring length alone cannot enforce a block round during membership changes; a singly linked ring cannot remove an arbitrary member in constant work without extra authority or a priced deferred mechanism; and stable residency does not make a Step cost only one header update. These are new design requirements derived from the sketch, not claims that the released code has these defects.

## 1. Semantic and Physical Contract

### 1.1 Agreed requirements

| Surface | Required direction | Boundary |
| --- | --- | --- |
| Autonomy | Runtime discovers and executes work itself. | No keeper, external intent executor or bot is required for ordinary progress. Existing authoritative data producers keep their own upstream role. |
| Program | One Actor may run a bounded sequence of Steps in separate blocks. | Keep its economic responsibility together; preserve already committed Steps. A whole-program atomic recipe is not the new default. |
| Current inputs | Each admitted Attempt reads the required authoritative current state. | Preserve exactly `Fixed` and `Percent` on valid typed Task surfaces, including available shares where appropriate. `Percent(100%)` means all current Available capacity. |
| Errors and retry | Retain `AbortCycle`, `ContinueNextStep` and bounded `RetryLater`. | Retry is the same current Step/Cycle with fresh dynamic inputs, not a restart of the whole program. Admission deferral is not an executed failure. |
| Single process | One open Cycle and one logical current service obligation per Actor. | No deferred next-Cycle latch or concurrent future-start machine while a Cycle runs or retries. Multiple physical dependency registrations do not constitute multiple execution obligations. |
| Current-state activation | A notification is permission to check, not proof that execution is now applicable. | Intermediate external states may be missed and changes may coalesce. Do not retain exact historical Crossing semantics under an unchanged name. |
| Funding history | Remove `PercentageOfLastFunding` and its Actor-only accumulator. | Independent custody, verified credit, adapter security and source authorization remain. |
| Live residency | Near-term runnable continuations stay in the ring across Steps and, where valid, Cycle boundaries. | No remove/reinsert or successor rematerialization merely because Q1 advances to the next block. State, cursor, fees, outcomes and guards still change and must be priced. |
| Parking | Nonterminal idle Actors may leave ordinary service with a complete wake plan or an explicit timed review. | False now does not prove permanent uselessness or authorize deletion. Parking changes service membership, not custody or Contract ownership. |
| Delayed work | A known future Step/retry/recheck uses a bounded sleep index when retaining it in the ring would cause wasteful visits. | `not_before` remains a lower bound; delays under bounded capacity are visible. Waiting is not a new future Cycle. |
| Cleanup | Separate loss of execution authority from physical reclamation. | Only authorized/terminal generations are reclaimable. A parked Actor is not garbage; owner mutation and old-generation sweeping must safely coexist. |
| Essential safety | Determinism, authorization, custody, atomic current-Step effects, committed-prefix durability, bounded work/state and complete two-component Weight. | These are not available as performance sacrifices. |
| Physical realization | One measured hot/cold layout, loaded-context discipline, ring, wake/recheck index, sleep mechanism and reclamation path. | Concrete encodings are chosen in this release; broader tuning is not. |

This table records the task owner's direction in the conversation. The normative released specification [S2] remains the historical contract until N0 replaces its affected sections explicitly.

### 1.2 Decisions to close in N0, not options to keep forever

| Decision | Working rule / required resolution |
| --- | --- |
| Ordering | Adopt persistent cyclic service with FIFO admission to the ring and preserved encounter order. This is not automatically the old global FIFO of per-Step tickets. Write the exact changed promise and counterexample behavior before production integration. No cheap-task, class or fee-based priority is implied. |
| Q1 and block boundary | Preserve at most one committed Step per Actor per block. Use one shared block-round boundary across all Actor passes. As a proposed conservative default, new/reentered memberships and source activity first observed in block B cannot receive service before B+1. Validate this against reference configurations; any different timing rule needs an explicit decision. |
| Live-head resource refusal | A runnable eligible head whose complete transition does not fit keeps priority for the next permissible pass/block; do not rotate it away to execute cheaper followers. A certified waiting/removal transition may detach a non-runnable head under the new specified ordering contract. |
| Recurrence | Name supported one-shot, level-sensitive repeated and coalesced-change-driven start modes. Persistent truth must not produce unbounded same-block starts. A busy Actor acquires no separate future-start obligation. |
| Short versus long wait | Prefer retaining a continuation due by the next permitted round. Put known longer waits into the sleep index. Fix the deterministic threshold and handling of unexpected future eligibility; do not use guessed probability as execution authority. |
| False preconditions | Preserve the existing skip/advance behavior unless separately changed. Start-condition false, Step-condition false, input unavailable and retry failure are different results. Do not park a running Cycle merely because its original start condition is now false. |
| Opening snapshots | Remove `PercentageAtOpening`, Opening-timed predicates and their snapshot machinery. Retain exactly `Fixed` and current-Available `Percent`; `Percent(100%)` replaces `AllAvailable`. Reject, rather than reinterpret, every historical encoded removed form. |
| Trigger family replacement | Map each useful old configuration to current-state start/review behavior or an explicit unsupported form. Exact transient crossing, sender/history-dependent activation and notification completeness are not equivalent to current-state polling. |
| Time | Choose supported clock domains, precision, intervals and overload/catch-up behavior. Coarse timing is allowed only with an explicit semantic limit. No arbitrary fixed number of period classes is imposed by this plan. |
| Manual / busy controls | Preserve authorization. Specify redundant Manual behavior while live/busy, paused, parked and retrying; it cannot silently reset retries or purchase a second Cycle. |
| Billing | Price notifications, negative checks, executed Attempts, refusal, retained state and cleanup. Reconcile removed Trigger/Opening fees instead of carrying their old names mechanically. A deposit is not reserved future CPU/PoV. |
| Parking versus disablement | Define autonomous wakeable parking separately from owner-paused/disabled state. Immutable/System controls keep their actual authority rules; a sweeper gains no mutation authority. |
| Atomic recipes | Not required for this release. Do not replace per-Step retry and prefix semantics under the cover of optimizing the ring. |

Close these as a finite decision table and executable oracle. Do not expand the runtime with every possible policy to avoid selecting one. Record additional approval when a decision goes beyond the stated mandate.

#### N0.2 frozen capability and transition matrix

The following table is the finite input to N0.3. It selects semantics, not a physical carrier.

| Profile | Start and recurrence | Current Step inputs and errors | Waiting and return |
| --- | --- | --- | --- |
| Burn Actor and liquidity/splitter families | Level-sensitive balance start; an exact account/asset notification coalesces a check. Completion may start another Cycle no earlier than the next block round only if the current condition still holds. | Current predicates; `Fixed` or `Percent`; existing `ContinueNextStep`/`AbortCycle` policy. Market `RetryLater` retries the same Step with fresh state. | Complete balance/spendability wake coverage parks indefinitely; otherwise timed review. Next-round continuation remains live; later retry sleeps. |
| Fee Sink Actor | Cadenced level-sensitive start at its configured deadline; missed periods coalesce to one current check and do not replay historical intervals. | Current threshold and `Percent` of current Available; `AbortCycle`. | Sleep until the next cadence deadline. Overload delays service without creating catch-up Cycles. |
| One-shot temporal User | `AtTime` becomes eligible once at or after its deadline. | Current predicates and amounts at the actual Attempt. | Sleep until due; after the single completed/terminal Cycle it has no autonomous recurrence. |
| Manual User/System Actor | An authorized Manual request coalesces to one activation check. A request while Running, Sleeping retry, Pending or already Live creates no second Cycle and does not reset retry state. | Current predicates and amounts; authored error policy. | Manual is direct bounded lookup. A paused/disabled Actor rejects ordinary Manual activation; the authorized resume/control path is separate. |
| Balance-reactive User | Level-sensitive current balance/spendability condition over exact declared surfaces; sender identity is not activation history. | Current predicates and amounts; authored error policy. | Exact complete invalidation parks indefinitely; hosts lacking complete spendability notifications use timed review or reject the profile. |
| Observation-reactive User | Level-sensitive current typed observation, or coalesced-change-driven reevaluation where every intermediate value is explicitly non-semantic. Exact transient Crossing is unsupported. | Current value/validity/age at check and Attempt; unknown, unavailable, uninitialized or stale is false for activation and typed failure where a Step requires valid input. | Exact feed invalidation plus validity/age deadline; incomplete domains use timed review. |
| Representative multi-Step/retry User | Start follows one of the supported families above; while busy no future-start obligation is acquired. | At most one committed Step per Actor per block; prior committed prefix survives; false Step precondition skips/advances; temporary failure may retry the same cursor. | Due by the next permitted round remains Live; later known eligibility sleeps; start-condition changes never park an open Cycle. |

| Decision surface | Frozen rule |
| --- | --- |
| Amounts | Exactly `Fixed(value)` and `Percent(perbill)`. `Percent` uses widened floor arithmetic over current Available at every Attempt; dynamic zero skips; positive `Fixed` above current capacity is funding unavailable and is never clipped. `Percent(100%)` is semantically all current Available. |
| Predicates | All admitted predicates are Current. Remove `ObservationTiming`, Opening predicate results, `OpeningSurface`, Opening amount snapshots, `PercentageOfLastFunding`, its accumulator and funding snapshot. Independent custody/credit authorization remains. |
| Ring order | FIFO admission into a persistent cyclic ring, then preserved cyclic encounter order. A resident continuation keeps membership; it does not publish a successor ticket. This deliberately replaces global FIFO by per-Step ticket. A resource-blocked eligible head retains priority; a certified non-runnable transition may remove it under the state rules. |
| Block round and Q1 | One shared round frontier spans all Actor passes. At most one Step commit per Actor per block. Membership first admitted or reentered in block B is ineligible until B+1; removal cannot donate a turn, and reentry cannot reset the same-block guard. |
| Short/long wait | Eligibility by the next permitted round stays Live. Any known later block/tick enters the bounded sleep index. Unexpected future eligibility without complete notification uses a deterministic timed review, never guessed readiness. |
| False/unknown | False idle start produces no Cycle and refreshes a valid park plan. Unknown/stale cannot certify indefinite parking: use the typed dependency deadline/retry or timed review. False Step precondition skips and advances. Temporary execution failure follows authored retry; permanent failure follows authored Abort/Continue policy. |
| Nonterminal return | Live remains Live for next-round work, moves to Sleep for a known later eligibility, or Parked only when idle with a complete/timed wake plan. Due Sleep and invalidated Park create one coalesced Pending check; successful current check admits Live no earlier than the next round. Pause/disable requires authorized recovery; retirement can only proceed through generation-safe reclamation. |
| Recurrence | Supported modes are one-shot temporal, level-sensitive repeated, coalesced-change-driven and authorized Manual. Persistent truth cannot start twice in one block. Busy Actors do not remember another occurrence; after completion current state is checked no earlier than the next round. |
| Controls | Create/update/pause/resume/close authority remains actor-type/mutability-specific. Manual is activation intent only: redundant intent is idempotent and cannot reset cursor, retry count, fees or round guard. Parked is autonomous and is not paused; cleanup has no mutation or custody authority. |
| Fees | Charge independent bounded owners for notification/invalidation, current negative/positive activation check, live/sleep/park transfer, admitted Attempt/effect, resource refusal and reclamation. No old Trigger/Opening fee name survives mechanically; no fee buys a second Cycle or reserves future execution capacity. User paths pre-admit their complete selected transition or fail without partial semantic mutation. |
| Cleanup | Terminal/authorized retirement revokes execution first, then bounded generation-bound sweeping reclaims physical state. Parked/Sleeping/Pending are not garbage. Custody remains sovereign and unchanged by service or cleanup transitions. |
| Unsupported legacy forms | Reject historical SCALE forms for `PercentageAtOpening`, `PercentageOfLastFunding`, `AllAvailable`, Opening timing, exact `ObservationCrossing`, and sender/history-dependent `AddressEvent`; do not decode them into new meanings. `ObservationChange` maps only to coalesced current reevaluation, `Cadenced` to deadline recurrence, `AtTime` to one-shot time, and `Manual` to authorized intent. Legacy broad AddressEvent is admitted only when certification can derive exact current dependencies and a complete or timed wake plan. |

No additional approval is required for this matrix: it applies the agreed current-state mandate and the separately approved two-mode amount/Opening removal. Any later proposal for transient event history, a second pending Cycle, priority classes, Opening snapshots, or a different same-block timing guarantee reopens task-owner approval.

### 1.3 One canonical process, different memberships

Logical service states describe ownership; they do not mandate a new stored enum for every word below.

| State / role | What is retained | Valid next transition |
| --- | --- | --- |
| **Live** | Canonical current process and one live membership; may be idle but due for its next start check. | One metered turn: Step/check, remain live, sleep, park, disable or retire. |
| **Sleeping** | Same current process and one future eligibility/review obligation. | Due extraction makes it eligible for live service or an activation check according to whether a Cycle is open. |
| **Parked** | No open Cycle requiring continuation; current Contract, custody identity and a bounded wake/review plan. | Relevant invalidation or a scheduled review creates one coalesced activation-check obligation. |
| **Activation pending** | A parked Actor needs a current-state check; no new Cycle is yet admitted. | False refreshes/replaces parking evidence; true installs one live membership; capacity refusal retains the check obligation. |
| **Paused / disabled** | Explicitly non-serving policy or process state with declared owner/protocol recovery. | Only the specified authorized transition; ordinary hints do not override pause or restore revoked authority. |
| **Retired / reclaiming** | Execution authority revoked; bounded old-generation state and cleanup cursor. | Owner or protocol cleanup; no automatic execution revival of partially deleted state. |

An open retry is Sleeping or Live when due, not Parked under a start-condition certificate. Releasing current execution state is never a side effect of a false start predicate. Fault/quarantine state must be typed and visible rather than disguised as normal parking or absence.

For each Actor/generation there is exactly one canonical residence/service owner. Dirty flags, reverse registrations and summary bitmaps are derived index authority with explicit consistency rules. A notification may invalidate a certificate while the Actor is still physically in the parking index; that transient state means **recheck owed**, not a claim that the condition remains false.

### 1.4 Persistent ring and block-round safety

The semantic reference model may take an ordered snapshot of eligible memberships at a block-round boundary. This is an oracle technique, **not** permission to enumerate or rewrite every live Actor at each block. Production must realize equivalent bounded service using frontier/round/membership authority.

Required invariants:

1. **Resident continuation:** after a successful Step that can continue next round, update required execution state and the service cursor without deleting/recreating the Actor's live membership or allocating a successor ticket solely for that progress.
2. **One block-round:** Prepass and later Actor passes share the same round identity/frontier and progress. Stopping for resource limits and resuming does not start another round.
3. **No duplicate turn or commit:** a member receives at most the specified one ordinary turn per block-round, and Q1 independently protects Step commits. Define paid resource-prefix retries across phases explicitly; they cannot become unbounded inspections or duplicate Attempts.
4. **Membership cut:** new or reentered memberships do not consume the turn of a member removed from the original round. Remove/reinsert, generation change and ring wrap cannot reset an Actor's same-block guard.
5. **Mutable round:** deletion of head/tail/interior members, cancellation, parking, sleeping, retiring and additions must preserve the remaining order without skipping an eligible survivor. A saved `len` or a sentinel that may be removed is insufficient by itself.
6. **No cheap-head bypass:** a live eligible head blocked only by insufficient remaining resources is retained; no class/Task/fee affinity is introduced. Certified absence of current work is a separate paid transition that may detach it under the approved new contract.
7. **Bounded work even without effects:** empty/stale nodes, already-served nodes, ineligible entries and failed transfers have bounded paid traversal. Physical visit limits are explicit; guards cannot create an infinite loop around a singleton ring.
8. **Fresh state:** residency retains scheduling locality, not yesterday's balances, quotes or eligibility decisions. Every admitted Attempt follows the current-input and mutation/rollback rules.

A supported Actor's complete transition must fit the applicable full-block service policy, or its input/profile must be explicitly inadmissible before becoming a permanently impossible live head. Changed runtime bounds/Weight require bounded recertification or a declared safe non-serving disposition; do not hide a permanently oversized transition behind ordinary temporary deferral.

A possible production technique combines a block-round frontier, membership generation/admission sequence and persistent per-Actor progress guards. This is a design candidate, not prescribed extra storage. Prove the chosen construction against the reference model and measure its bookkeeping.

Append/wake admission order, delayed return order and the relation between an unserved remainder and previously served residents must be deterministic. Persistent round-robin residency does not by itself reproduce the old future-ticket FIFO; N0 names precisely what is preserved and what changes. “Likely to execute soon” describes the intended working set, not a fixed latency promise under a large live population.

### 1.5 Parking evidence and wake completeness

A park decision needs a **Park Certificate** in the logical sense: a compact runtime-owned reason why ordinary reevaluation can wait, plus the authority that ends that wait. It need not be a signature, large proof object or new public type. It binds the current Actor generation, accepted start/read plan, applicable dependency state and any time validity limit.

Two admissible forms:

- **Complete event-driven parking:** every supported change that can invalidate the negative conclusion or create promised service reliably marks the Actor for bounded reevaluation. No periodic polling is required for the closed dependency domain.
- **Timed-review parking:** complete notification coverage is unavailable or too expensive; retain an explicit next review deadline in the bounded runtime service mechanism. Polling cost and delay remain part of the result.

Do not create two permanent engines for these forms. They are two wake plans in one service design.

Completeness covers the actual predicate/read domain: balances and spendability, holds/locks/freezes, fee reserve/protected minima, staking shares, authoritative observations and their validity/age, time and runtime/configuration changes where relevant. Check balance decreases as well as increases when they can change the predicate. Notification coverage for asset transfers alone is not coverage for `AvailableNow`.

Use exact dependency keys, not only a coarse BALANCE/OBSERVATION type bit. A generation-bound read plan may conservatively watch all relevant dependencies. Smaller falsity witnesses for AND/OR are optional later optimizations; do not enumerate all truth assignments or invent an expensive solver for the first release.

No supported complete-park path may silently miss a wakeup. If a host cannot provide completeness, explicitly choose timed review or reject that profile. Unknown/stale/corrupt state is not proof of safe indefinite parking. Preserve typed failures and bounded recovery.

### 1.6 Indexed parking and coalesced activation without cause history

Canonical Actor data remains under stable bounded ownership; parking indexes hold references and wake metadata. Organize lookup by concrete dependency or activation class as appropriate:

```text
sovereign/account + typed balance surface -> matching parked registrations
feed or shared dependency                -> bounded parked subscriber pages
manual / owner control                   -> direct authorized Actor lookup
time / fallback review                   -> sleep/review bucket
```

An account-to-Actor relation must be real registered authority; do not assume a hashed sovereign address can be inverted. Actor-local direct lookup may avoid a subscriber list. Shared dependencies must not cause unbounded fanout in the source mutation.

A bounded notification records only that a check is owed. Repeated notifications coalesce; while already pending they do not append more checks. During an open Cycle they create no next-Cycle promise. Source state can still change and the next current Step reads it. A positive activation check proves applicability only at its defined observation point. Ring admission is not a durable promise that `start_when` will still be true later: recheck at actual Cycle start or prove the carried authority remains fresh. Price any second check and do not recreate a paid historical readiness latch under the name “activation”.

**Lost-wakeup invariant:** an update arriving before, during or after negative evaluation, registration, dequeue, callback or acknowledgment cannot disappear between clearing `pending` and going back to sleep. Use an explicit atomic/version protocol: install or validate watched revisions, evaluate, then acknowledge only the version actually covered; a later change leaves pending work. Handle replay/rollback, registration replacement, same-block updates, generation reuse and revision exhaustion. A plain `dirty = false` after a check is not a proof.

If a feed-level dirty cursor amortizes wake discovery, updates during partial traversal, new/removed subscribers and notifications behind the cursor must remain discoverable. Pending-queue saturation cannot drop an invalidated certificate: retain durable/coalesced source or Actor-level work with a bounded recovery rule. This obligation does not require one stored event per historical source update.

### 1.7 Residency transfers, idle locality and anti-thrashing

Transfer service membership, **not the complete Actor**. Contract, custody and canonical continuation are not serialized into a parking silo and copied back. Hot/cold partitions may be physically reorganized when justified by co-access evidence, but mere service-state changes do not require full body movement.

For Live -> Sleep/Park, Park -> Pending -> Live, cancellation and retirement:

- pay for inspection and reserve the complete selected transfer before semantic mutation;
- establish/reserve a valid destination or explicit durable pending authority before dropping the old obligation;
- commit canonical residence and all required indexes atomically; on failure preserve exactly one obligation;
- provide bounded head, tail and **arbitrary-member** removal for owner control and generation replacement;
- do not search the entire ring for a predecessor. A singly linked carrier needs a proven predecessor mechanism or a bounded, fully priced lazy alternative; a doubly linked/paged carrier has its own write costs;
- bound stale memberships and retained generations. Tombstones may exist only as an explicitly priced bounded mechanism, not an unaccounted substitute for unlink;
- do not let observer-only API calls or unsupported helpers become alternate membership writers.

No background pass scans the live ring merely to decide who should be parked. Make that decision during an already owed Actor turn or explicit lifecycle transition.

On Cycle completion, an idle Actor may remain resident until its next allowed start check. If a fresh, already available context safely proves continued live service or a valid park plan, use it; otherwise prefer one bounded later check to Park -> immediately reactivate churn. Do not commit a second Cycle or use a stale cached balance just to avoid a move. Fix the maximum such live grace in the service policy.

Unrelated changes cannot wake a parked Actor; repeated relevant changes cannot duplicate its pending entry. A false activation check normally refreshes parking evidence without rewriting unchanged registrations. An already-live next-Step/retry due at the next permissible round need not leave the ring. Longer waits use the sleep index rather than paid repeated live visits. Owner-disabled and retired Actors do not follow ordinary wake hints.

**Optimization hypothesis to prove:** useful consecutive turns require fewer physical membership operations than repeated dequeue/successor publication. Ring-head writes, predecessor/tail repairs, per-Actor guards, negative checks and cold-state reads still count. Do not report “Step -> head = next” as the total economic or resource cost of a Step.

### 1.8 Physical closure here; tuning later

| Mandatory function in `0.7.27` | Not mandatory now |
| --- | --- |
| Freshness-safe loaded Attempt context; an adequate derived Step/Wake read plan | A new VM, JIT, persistent global value cache or every specialized shape |
| Stable canonical ownership, co-access-informed hot/cold geometry | Broad AoS/SoA, key-locality, packing and hasher sweeps |
| Correct block-round/live membership and bounded arbitrary control | A particular intrusive layout or globally optimal ring page width |
| Exact-key parked lookup, coalesced pending checks, complete or timed-review wake plans | A universal dependency graph or a second causal event history |
| One concrete sleep/review structure with saturation/wrap/recovery coverage | Exhaustive radix/timing-wheel/fanout competition |
| Safe disablement, generation-isolated reclaim and bounded debt | Maximum sweeper throughput or every possible batching variant |
| Metered selectors/transfers, actual production Weight and end-to-end cost | Cosmetic coefficient records without a changed useful service boundary |

Choose every installed encoding and bound now. Defer its broad optimization, not its existence or coverage. Do not add a technique just because it appeared in the previous brainstorming catalogue.

## 2. Historical Evidence Without Historical Lock-In

### 2.1 Keep the records; change how they are consumed

Keep historical experiment IDs, paths, original decisions, measurements and validity limitations. Do not renumber sealed IDs, restart the allocator, bulk-mark the corpus `Rejected`/`Invalidated`, or relocate every historical file. A semantic redesign can make an experiment irrelevant to the new contract without making its original result false. [S3]

Start a **new semantic lineage** within the existing Actors track. Use a stable descriptive identifier such as `current-state-v1` only as a planning label until the accepted specification binds its exact identity. A semantic lineage is not automatically a new folder, track, global registry or framework subsystem.

The Actors track entrypoint should show, in order:

1. Current accepted/proposed semantic contract and active release proof obligations.
2. Explicitly imported prior claims, with scope and current owner.
3. Historical lineages and their existing records, fully discoverable but not all rendered as current work.

The active graph contains new mandatory obligations plus deliberately imported prerequisite claims. `Uses evidence from`, provenance links and ancestry do not import every historical hard dependency recursively. A reused theorem still needs its actual premises justified; a reference to the old experiment is not that justification.

### 2.2 Historical result and current applicability are independent

Use the existing record decision vocabulary for the original decision. Add a compact **current-lineage applicability projection**, preferably in the existing track index or owning current record, rather than retrofitting every old file.

| Current use | Meaning | Release effect |
| --- | --- | --- |
| **Reuse — scoped** | The exact claim's premises, mechanism and required identities still hold, or an explicit bridge proves applicability. | May discharge only the named new obligation. Numerical Weight and service conclusions need quantitative applicability, not just qualitative similarity. |
| **Revalidate — required** | The mechanism or claim survives, but relevant semantics, domain, code or artifact identity changed. | A bounded new proof is required before relying on it. |
| **Hazard transfer** | An old finding exposes a failure mode that may recur in retained or replacement code. | Carry the failure scenario into the new safety inventory; do not import the old implementation as a mandatory design. |
| **Historical — no active dependency** | Its claim belongs to removed semantics, an unselected mechanism or a past campaign target. | Preserve the record and lesson; no release gate or rerun follows merely from its existence. |

Unassessed old records are **not imported evidence**. The initial pass must identify known safety findings and retained cross-domain dependencies, but does not require a fresh verdict on every old experiment. Detailed assessment happens when a current claim consumes an old result or a retained path makes an old hazard relevant.

A minimal imported-claim row contains: `old ID + claim/section → old premises → relevant change → current use + reason → current obligation/owner`. If using a selected claim from a compound old index section, extract that claim with provenance; do not first refactor every unrelated historical section.

### 2.3 Initial routing examples, to verify rather than assume

| Historical evidence | Initial route for the new model |
| --- | --- |
| C1 selection and A0 exact transition oracle | Historical architecture/old semantics. Retained invariants become inputs to new tests. The entire old transition digest is not a conformance gate for intentionally changed behavior. |
| Useful Trigger / pending next-Cycle latch / rearm while Running | Historical where these transitions are actually removed. New single-Cycle lifecycle and reevaluation behavior need new proofs. |
| LastFunding accumulation and source filtering | Historical for the deleted Actor amount mode. Reuse or revalidate independent custody/ingress/authentication claims that still apply. |
| Opening snapshots and old 12/24/48 joint geometry | Conditional on the N0 decision. Unused old geometry is not a mandatory numerical target; retained snapshot functionality keeps its coverage obligations. |
| Atomic Task effects, committed-prefix durability, retry outcome boundaries, custody, authorization | Hazard transfer and likely scoped reuse/revalidation. These are retained obligations, not automatically valid implementations. |
| EXP-0113/0114 selection-owner gap and fix | Preserve the general failure scenario: pure control flow needs a measured owner. Import the numerical coefficient only if the actual measured selector still applies. |
| EXP-0117 deep temporal coverage/full-capacity findings | Hazard transfer immediately. If the same cursor survives for retry, idle reevaluation or sweeping, qualify that domain. If fully removed, prove elimination and replacement safety; do not repair the old cursor just to close its historical record. [S4] |
| EXP-0118 rejected multi-way cursor | Historical narrow outcome and optional candidate insight, not a ban on a new design. No mandatory encoding experiment until the new service contract actually needs that structure. [S5] |
| Old P7/P8 percentages, sensitivity rankings and fixed cohorts | Diagnostic context. Not current ranking, a guaranteed saving or a complete new workload contract. Recompute only consumed premises under matching identities. [S6] |
| PR #32 tail/non-tail selection failure | General hazard transfer: selector eligibility must imply executor-domain validity. Preserve the concrete regression if that code survives; apply an equivalent test to replacements. |

### 2.4 Eliminating a legacy hazard is a proof, not a label

An inherited safety obligation may close for `0.7.27` through any of:

- a confirmed repair and current regression;
- a valid scope/reachability explanation refuting the apparent counterexample;
- verified removal from every supported execution/configuration path, plus coverage of the replacement responsibility;
- an explicitly approved and enforced support restriction, with no silently accepted unsupported configuration.

For elimination, inspect production calls, genesis, runtime hooks, adapters, generic embedding, feature builds, supported upgrades/migration code if any, authoring inputs, generated descriptors and maintenance/recovery entrypoints. A `legacy` module or fallback that remains reachable keeps its safety obligations. Stored old state is not evidence of elimination when a supported path can revive it.

Keep the historical finding's original status and scope. “Not present in the new release” does not mean “the old release was correct.” If safe old-line remediation is independently needed, name that work separately rather than either hiding it or making full old-engine modernization a prerequisite.

### 2.5 Known findings to route at intake

These IDs come from the original supplied plan and are not reserved EXP numbers. They make the handoff self-contained; verify applicability rather than assuming each finding is a defect in the new model.

| Finding | Intake responsibility |
| --- | --- |
| H1 — deep temporal coverage | EXP-0117 challenges deep Retain/Remove coverage under its limited fixtures. Validate complete reachable paths and total accounting if inherited; otherwise prove elimination and replacement coverage. [S4] |
| H2 — full-capacity rearm | Classify ordinary capacity exhaustion versus corruption/worker poisoning, and preserve a bounded current obligation or defined refusal/recovery in whichever new index survives. [S4] |
| H3 — narrow multi-way rejection and wording | Keep EXP-0118's historical outcome. Its threshold and sign-error prose do not decide new-layout selection; requalify any consumed quantitative premise. [S5] |
| H4 — sensitivity inconsistency | Reconcile treatment of freed earlier charges before reusing the old removal/partial-reduction model. Test any new counterfactual model used to rank candidates. [S6] |
| H5 — primary evidence inside the index | Extract independently decidable evidence only when a current obligation consumes it. New primary measurements belong to Leaves, not the growing entrypoint. [S3], [S6] |
| H6 — transient observations/prototypes | Label missing historical bytes. Preserve essential new samples, traces, fixture identity and prototype diffs rather than only a digest of unavailable output. |
| H7 — mixed resource boundaries | Requalify old reservation, settlement, physical-access and workload percentages. Their magnitudes do not rank the new engine. |
| H8 — selector/executor mismatch | Carry the PR #32 tail/non-tail failure scenario into every retained or replacement domain-specific selector; do not label the fixed Crossing bug still unfixed. [S7] |
| H9 — stale current summaries | Correct the current backlog/index/binding projection; preserve the published tag and distinguish absolute blocks, horizons, queue entries and current artifact identities. |

A rejection of a new candidate never restores authority to a contradicted baseline. A known applicable finding always has its own closure owner, regardless of the optimization campaign's stopping rule.

## 3. Baselines and Honest Comparisons

| Reference | Purpose | Restriction |
| --- | --- | --- |
| **H — Published `v0.7.26`** | Immutable historical delivery and economic-use-case reference | Verify commit `c16675edf03e3af0d6dcf3080a31c64b49765a5b` and its recorded artifacts. Challenged old coverage remains qualified. [S1], [S7] |
| **L — Legacy overlap bridge, only where needed** | Check a surviving mechanism or compare a shared supported scenario | Repair/remeasure only the comparison-critical or retained domain. A complete optimized legacy engine is not required. |
| **CS0 — Sound implementation of the accepted new contract** | Native same-semantics reference for physical choices | Prefer a pinned minimal complete vertical slice already needed for development. Do not manufacture a deliberately slow strawman or a second permanent executor. |
| **CS1 — Selected release implementation** | Final new-model result | Compare CS1/CS0 on the same contract. Compare CS1/H separately by economic goal and explicitly list changed guarantees. |

A reference model for semantic tests is not a throughput baseline unless it uses representative production execution. If no separate CS0 optimization is warranted, report the new model's measured result without inventing a CS1/CS0 speedup.

Separate physical efficiency, change in admitted timing/order guarantees, resource-policy redistribution, pricing correction and eliminated capability. Preserve negative/censored results. Do not compare “Steps/block” without verifying that both Steps represent the same economic work. Use successful effects, completed intended workflows, resource cost, detection/progress/completion delay, pending work and maintenance debt.

The benchmark host remains the user's interactive workstation. Use finite matched same-host comparisons and retained raw evidence under the Benchmark Reassessment Protocol. A fresh noisy number is not automatically stronger; an old contradicted coefficient is not protected by the noise of a new run. Empirical bounds require justified measurement scope and conservatism; a maximum of a few fitted medians is not a universal CPU theorem.

## 4. Execution Order, Campaign Bound and Meaning of Completion

Task IDs name workstreams, not an instruction to implement the whole kernel before defining comparisons.

```text
N0 exact current-state + ring/parking semantics and oracle
  + N1 hazard/applicability intake
  + N6.1 workloads and materiality before decision measurements
                              |
                              v
N2 minimal multi-Step/current-balance/retry kernel
  + N3.6 independent round model
  + N3.3/N3.7 wake/parking protocol model
                              |
                              v
N2.5/N2.6 Step-Wake plans + co-access/initial geometry
  + N3.1–N3.8 one live/sleep/park/pending implementation
  + N4 disablement/mutation/reclaim
  + N5.2 path-by-path resources
  + N6.2 bounded necessary carrier choices
                              |
                              v
N6.5 residency/thrashing/whole-maintenance evidence
  + N5.1/N5.3/N5.4 removal, hazards and physical closure
                              |
                              v
N6.3 final whole-service comparison -> N6.4 design decision
                              |
                              v
N7 final artifact set, independent review, assurance and publication
```

Begin known safety triage immediately. A working provisional carrier may support the vertical slice, but cannot become the release geometry merely by being implemented first. Do not begin with a complete old-engine repair, full historical DAG migration or universal physical optimization sweep.

**Finite campaign:** freeze the smallest adequate shortlist and finite refinement allowance for each necessary design choice. Keep one active candidate on a shared hot path. Coupled pieces may form one candidate when their necessity is stated. Test a simple adequate carrier before adding indexes to save costs that have not been observed. Not every task needs a new Experiment ID or multiple prototypes.

**Normal release acceptance:** close the agreed semantic and physical responsibilities, all known supported-domain safety/resource blockers, and demonstrate a predeclared materially useful end-to-end gain on at least one representative core economic goal. Compare the new contract's physical choices on equal semantics; compare against the old release through a labelled capability/timing/order bridge. Count the whole cost of detection, negative checks, sleeping, mutation and reclamation. A smaller queue-operation coefficient alone is not enough.

There is no mandatory `100/block`, `2×` or universal percentage. Define materiality before selecting on results. Do not require every workload to improve. Separate safety repricing, changed guarantees, resource redistribution and actual work removed. Old due-frontier isolation already limits what may honestly be claimed as a new scalability gain: do not assume the previous scheduler scanned every existing Actor without evidence.

If the finite campaign fails the materiality criterion, record its result and the exact remaining decision. Do not endlessly add candidates or claim a gain. Publishing a semantic/correctness-only checkpoint requires a distinct task-owner disposition; the default release goal is unchanged.

**Freeze:** after N5.4/N6.4 choose one production service/physical design. N7 admits only required correctness, soundness, binding, review and public-truth corrections. Independent performance opportunities belong to the proposed `0.7.28` portfolio unless explicitly reauthorized.

## N2 — Build the Current-State Execution Core

- [ ] **N2.5 / Certified Step and Wake Plans.** Reuse or extend admission certificates for immutable Step structure and idle start dependencies: exact typed read surfaces, relevant mutation/time boundaries, outcome shapes and resource selectors. Bind Contract/body, generation where applicable, semantic/physical versions, bounds and Weight identity. Derive on create/update or bounded recertification. Park evidence may refer to this plan; do not duplicate the authored program or add a universal dependency VM. Unsupported host wake domains select timed review or explicit refusal. The retained certificate already commits Contract/body, semantics, production Weight, geometry, configured bounds, lifecycle Weight and per-Step control/effect envelopes, and central control projection now refuses an internally valid certificate after host authority changes. The wake audit found six admitted Trigger families but no independent host wake language: Manual has no autonomous dependency; AddressEvent depends on certified host movement ingress plus Contract source/asset filters; ObservationChange depends on one typed feed/revision fanout; ObservationCrossing depends on one typed feed, current observation revision/value, phase and threshold membership; AtTime/Cadenced depend on the shared Tick clock, temporal anchor/runtime state and exact wake reference; schedule windows add Block time and terminal wake authority. Existing Contract/admission commitments, trigger runtime state, activation authority and generation-bound wake references together carry those selectors, but no single qualified plan is consumed across every check/wake path, and no bounded recertification behavior exists yet. Admission now derives one compact typed wake qualification containing Trigger family plus domain-separated selector and schedule commitments, binds it into admission identity, and exposes fail-closed wake projection; focused falsifiers prove family or selector mismatch cannot authorize either Unsignaled or temporal Waiting control authority. The Manual production ingress now consumes the qualification through the canonical active-Contract load and classifies a separately valid but family/selector/schedule-mismatched certificate as corrupt before Trigger admission. Remaining work must route each automatic detector and temporal wake path through equivalent qualified authority, explicitly refuse unsupported future domains, and measure certification cost; do not add recertification unless fresh-genesis refusal is insufficient. **Exit:** The current execution and wake/check paths consume one qualified plan; stale plans cannot authorize service or indefinite parking, and certification cost is measured.

- [ ] **N2.6 / Co-Access and Stable State Geometry.** On the vertical slice, map reads/writes and mutation frequency for resident Steps, retries, idle checks, admission, parking, owner update and sweep. Choose hot/cold partitions with stable Actor ownership; membership indexes hold bounded references, not copies of whole Contracts. Measure keys, value bytes, decoder work and update amplification. Keep live index cardinality bounded by supported occupancy, not the largest historical ActorId; recycled physical slots need generation protection. **Exit:** A concrete initial canonical layout and co-access explanation; necessary alternatives enter N6.2 before geometry freeze, not a broad layout sweep.

## N3 — Persistent Live Service, Indexed Parking and Bounded Wakeup

- [ ] **N3.1 / Autonomous Discovery and Activation Checks.** Implement runtime-owned discovery using bounded notifications, current dependency/source state and timed fallback. Hints authorize checks, not effects. A parked invalidation creates one generation-bound pending check; verify current start applicability before live admission. Cheap check and admission may share a transaction when fully priced, but logical roles remain distinct. Use direct registered account lookup where appropriate and bounded shared-feed traversal; neither whole-population per-block scanning nor unbounded per-source fanout is an acceptable implicit fallback. **Exit:** Work is discovered without external executors; all negative and pending work is accounted, overload assumptions are explicit and promised checks are not dropped.

- [ ] **N3.2 / One Current Obligation and Residence Policy.** Unify live continuation, delayed retry and idle review under the accepted recurrence/phase rules. Keep near-term next Steps resident; do not generate old-style successor wakeups solely to wait one Q1 block. Ignore future-start hints while a Cycle is open. On completion retain the Actor for its next permitted check or safely transfer once using fresh information; avoid Park -> immediate Live churn. Specify short-wait residency and longer Sleep transitions without probabilistic eligibility. **Exit:** Recurring policies, multi-Step Actors and retries advance with one service owner; no same-block restart loop, double Cycle or unnecessary per-Step topology migration.

- [ ] **N3.3 / Certified Parking and Wake Completeness.** Implement logical Park Certificates under §1.5: generation/plan binding, a valid negative reason and every trigger/time boundary that ends its validity. Prove complete notification domains or use a concrete timed-review obligation. Start with conservative exact dependency sets; narrower DNF falsity witnesses are optional. Distinguish parked idle policies from open retries and owner-disabled state. Notification invalidation makes a check owed; it need not assert that execution is already possible. **Exit:** No live work is hidden by a stale/partial certificate; false-now never implies deletion, and every supported nonterminal wait has the promised bounded-cost return path.

- [ ] **N3.4 / Negative Work, Fairness and Griefing Budget.** Price all no-effect turns, notifications, duplicate hints, failed checks, admission-prefix retries and stale-reference visits. Define User payment/service-budget and System bounded-allocation responsibility. Test tiny-credit storms, toggling conditions, always-false/true programs, expensive eligible heads, fee/provider failures and sustained external demand. Busy hints cannot reset retries or accumulate next-Cycle readiness. Keep admitted service order and no-cheap-head-bypass rules explicit; repairable waits and resource insufficiency are distinct. **Exit:** No free unbounded polling/fanout, hidden priority or unbounded repeated prefix work; contention and liveness hold within declared capacity/provider assumptions.

- [ ] **N3.5 / One Concrete Ring/Sleep/Index Carrier.** Choose a minimal carrier using N2.6 and bounded N6.2 evidence. Fix ring headers/nodes/pages, removal authority, sleep/review buckets, pending checks, occupancy summaries, time range and generation rules. A singly linked list is not presumed O(1) for arbitrary control. Compare a second carrier only when necessary; do not implement an intrusive ring, paged ring and bitmap engine in parallel. Cover holes, wrap, full pages, clustered/overdue deadlines and corruption. **Exit:** One concrete bounded storage/index design with measured full/fragmented cases; common live progress does not allocate a new membership and all control operations have bounded authority.

- [ ] **N3.6 / Block-Round Frontier and Persistent Ring Service.** Implement the §1.4 oracle-equivalent round protocol across Prepass/Drain or their approved replacements. Track one block frontier/progress and independent Q1 protection. Prove behavior under zero/one/many members, budget interruption, tail/head/interior removal, new admissions, current-member cancellation and generation replacement. Newly eligible memberships cannot borrow removed members' turns. Do not rely on initial ring length, a removable sentinel or unlimited scanning of already-served nodes. Preserve unserved order and the specified live-head refusal behavior. **Exit:** No duplicate or skipped eligible turn, same-block reentry exploit, ring-wrap loop or phase-reset double service; a resident next Step needs no successor rematerialization.

- [ ] **N3.7 / Dependency-Keyed Parking and Lost-Wakeup Protocol.** Implement the exact-key registries/direct mappings, reverse ownership and bounded dirty/pending mechanism selected in N0/N3.3. Cover every supported balance/spendability/provider/time mutation or route that domain to timed review. Specify check/registration/acknowledgment ordering and a monotone/versioned recovery rule for updates during callbacks, partial feed traversal, subscription changes and rollback. Version exhaustion must fail safely, not alias an old certificate. Coalesce notifications without copying cause history. **Exit:** Deterministic adversarial interleavings cannot lose the only future check, publish two activations or wake an unrelated/old generation; notification cost remains bounded and fully attributed.

- [ ] **N3.8 / Membership Transfers, Arbitrary Control and Saturation.** Implement atomic Live/Sleep/Park/Pending transfers with destination admission or durable pending authority secured before releasing the source. Cover owner pause/update/cancel/retire of any ring position, stale generations, last-member deletion and full live/sleep/pending/parking structures. A normal full condition is typed deferral/backpressure, not corruption or permanent global worker poison. Bound lazy detach and stale debt if selected. Preserve exactly one current obligation after every rollback and partial-capacity outcome. **Exit:** No lost or duplicate membership, unbounded predecessor search, dropped invalidation or full-capacity terminal dead end; every transfer and its failure suffix has a complete resource owner.

## N4 — Disablement, Mutation and Asynchronous Reclamation

- [ ] **N4.1 / Revocation and Authorized Revival.** Implement Parked versus owner-paused/disabled versus retired status under the agreed contract. Safely revoke execution with bounded index work while preserving custody. Mutable owner update/reactivation follows generation and admission checks; Immutable/System rights do not expand implicitly. An interrupted Cycle is cancelled according to its existing semantic boundary, not rolled back economically. No ordinary wake can resurrect retired/partially deleted data or erase retry history. **Exit:** Cheap cessation of service and authorized mutation/revival are complete; nonterminal parked Actors are never mistaken for reclaimable objects.

- [ ] **N4.2 / Generation-Safe Owner and Protocol Cleanup.** Implement resumable bounded cleanup of authorized retired generations. Epoch/era may determine eligibility, never unbounded one-block work. Reserve state capacity for required revocation/retirement before it becomes mandatory, and define a funded maintenance budget under sustainable-load assumptions rather than idle-only progress. New Contract generations cannot write into or be removed through an old cleanup namespace. Detach old execution/index authority safely; generation tags do not erase their physical traversal costs. **Exit:** Owner mutation during sweep preserves the new program and custody; mandatory disablement remains possible at full capacity and cleanup progress is explicitly funded and bounded.

- [ ] **N4.3 / Retained State, Holds and Reclamation Debt.** Price parked, disabled and retired bytes; specify when hold/slot capacity is released. Bound old-generation accumulation, stale ring/index references and cleanup cursors under frequent update, close and revival. Enforce admission backpressure before debt becomes unsupportable. Test halted/underfunded maintenance, deadline bursts, interrupted sweeps and recovery. Separate storage collateral from block-computation reservations. **Exit:** No free unbounded cold storage, uncollectable cleanup debt or cost moved silently from live service into future generations.

## N5 — Remove Obsolete Duties and Close the Physical Resource Model

- [ ] **N5.1 / Old-Path Reachability and Removal.** Remove or explicitly isolate old next-Cycle latches, obsolete causal traversal/rearm, LastFunding-only state and per-Step successor publication replaced by ring residency. Inspect genesis, hooks, Router/TMC/Oracle/Staking ingress, APIs, clients, feature builds and independent embedding. Retire old snapshots only by a named semantic change; preserve independent custody/provider/effect invariants. No hidden old scheduler may remain as an unpriced fallback. **Exit:** Deleted duties are truly unreachable, inherited responsibilities stay protected, and only one supported engine owns each behavior.

- [ ] **N5.2 / Complete Weight Owners and Reachable State Domain.** Develop resource coverage alongside each path, then close it for the installed representation: create/certify, notifications, negative checks, ring rounds/guards, current execution, retry, arbitrary unlink, transfers, sleep extraction, parking refresh, owner update and sweep. Pay bounded read-only selectors before they run; cover all mandatory suffixes before mutation. Include singleton, maximum/fragmented/deep/full indexes, cold/hit context, refused and rollback cases. Separate requested/outstanding reservation, actual settlement, logical accesses, distinct keys and root-inclusive proofs. **Exit:** Every shipped owner/selector covers its legal domain without omissions, overlap subtraction, shallow-fixture assumptions or imaginary free CPU work.

- [ ] **N5.3 / Known Hazard and New-Model Soundness Closure.** Close N1.3 transfers against every supported reference/host domain. Verify selector-to-executor implication, generation freshness, callback visibility, no-lost-wake, ring-round invariants, custody and capacity refusal. If any old deadline/queue owner remains reachable, its applicable hazards stay active. Check generic host configurations; require timed fallback or reject unsupported wake semantics rather than making a universal notification claim. **Exit:** Zero unresolved known supported-domain safety/resource blockers, with exact reviewed outcomes rather than a claim that all imaginable future defects were disproved.

- [ ] **N5.4 / Physical Architecture Closure Gate.** Compose the accepted execution, ring/round, parked lookup, pending-check, sleep and reclaim design into one compact record. Name canonical state ownership, Step/Wake plans, freshness boundaries, concrete encodings/bounds, removal costs, transfer protocol, wake completeness/fallback and complete billing. Include N6.5 residency/churn evidence and show which ordinary topology operations were actually eliminated. The Synthesis owns no new raw benchmarks. **Exit:** One production-capable model has measured ordinary, negative, delayed, burst and maintenance costs; no essential round, wake, capacity or Weight proof is deferred to 0.7.28.

## N6 — Bounded Comparison, Residency Evidence and Final Decision

- [ ] **N6.1 / Economic Goals, Workloads and Materiality Freeze.** Start before candidate decision measurements. Freeze workloads that exercise real reference Systems and representative Users: multi-Step current balances, funded retry, level/change/one-shot recurrence, long live runs, sparse parked populations, wake storms, sleep/reentry, owner mutation, heavy effects and cleanup under demand. Fix populations, horizons, fairness/timing rules, outcomes, budgets and meaningful acceptance thresholds. Distinguish equal-new-semantics comparisons from old-to-new economic-goal bridges. **Exit:** A finite matrix and materiality rule can detect moved polling/cleanup bills, lost work and order changes; no arbitrary universal rate or post-result target.

- [ ] **N6.2 / Minimal Necessary Physical Choices.** Use the vertical slice, co-access map and round/wake models to select only necessary alternatives before freeze. Resolve stable state placement, ring carrier/removal authority, parking lookup/coalescing, sleep index and reclamation. An adequate reused implementation needs scoped evidence, not a competing prototype by ritual. Predeclare a small shortlist and finite refinement allowance for a real material choice. Do not optimize a node-local warm cache or chase every bitmap width. **Exit:** Each necessary choice is supported, rejected or bounded as adequate; one implementation remains and its claims separate removed work, improved admission, lifecycle tradeoff and changed semantics.

- [ ] **N6.3 / Final Whole-Service Production Comparison.** Compare selected new geometry against a sound same-contract reference where meaningful, and against H/L by the declared economic-goal bridge. Use exact final production-Wasm and uninstrumented confirmation. Report effects, intended workflow completion, delays, service gaps, censored/pending work, notifications/negative checks, retries, User/effect/Control Weight, state and cleanup debt. Include N6.5 churn and round costs. Validate any fixed-trace sensitivity arithmetic; do not sum overlapping local improvements or treat charged counters as independent physical bounds. **Exit:** Declared materiality and protected regressions hold on the whole retained service, or the finite campaign records its exact obstruction without a false speedup or indefinite extension.

- [ ] **N6.4 / Design, Sacrifice and Release Freeze.** Publish the composed accepted decision: what Actor means, ring-order/Q1 boundaries, live versus delayed/parked residence, current reads, preserved retries, lost transient/provenance guarantees, recurrence, billing and generation-safe cleanup. Explain physical choice and the measured price of moving work. Obtain approval for residual changed guarantees beyond §1.2. Freeze one engine; transfer only a compact measured residual ledger to the later optimization portfolio. **Exit:** The semantic and physical decision is complete, useful performance is established, and no unselected alternative becomes an automatic new 0.7.27 campaign.

- [ ] **N6.5 / Residency Benefit and Parking-Thrash Accounting.** Measure resident membership writes, round/header/guard cost, Step state persistence, park/sleep/live transfers, certificate and subscription churn, false wakes, wake latency, inactive-set traversal and sweeper debt. Compare long useful runs, repeated true/false conditions, bursty updates while pending, adjacent-block retries, long sleeps, interior cancellations and huge idle sets with a small live frontier. Track physical operations, not just status changes. An instrumentation-only legacy reconstruction is not a claimed current benchmark. **Exit:** Stable residency demonstrably removes the targeted republishing cost without adding a larger ring/polling/index bill; parking wins or its precise break-even limits are declared, and ordinary Step accounting is never reduced to a fictitious one-header-write total.

## N7 — Bind, Review and Publish the Actual Retained Design

- [ ] **N7.1 / Final Cutover and Artifact Identity.** Remove losing runtime code and unapproved compatibility paths. Retain decision-bearing diagnostic/prototype provenance separately. Generate complete affected production Weight and consumers under the reassessment protocol; bind benchmark/production Wasm, runtime semantics/versions, metadata, ABI, plans/bounds, fees, observation/ingress and clients. Existing owners survive only with exact current applicability. Noise cannot justify an invalid old price; freshness cannot validate a bad new fit. **Exit:** One exact final artifact set implements the accepted contract and selected physical design across the declared host/feature boundary.

- [ ] **N7.2 / Independent Round, Wake and Resource Review.** Independently challenge oracle and implementation: singleton/modified rounds, Q1 across phases, head refusal, reentry generations, lost wakeups, incomplete spendability hooks, dirty acknowledgment, stale loaded context, unauthorized revival, full-index transfers and update/sweep races. Review transferred hazards independently of optimization status. Test applicable cached/noncached paths for semantic equivalence without assuming equal Weight. Verify old-path elimination at all supported entrypoints. **Exit:** Every mandatory obligation has reviewed scoped closure and every unsupported profile is explicitly rejected; no previously missed domain is hidden by a successful common-path test.

- [ ] **N7.3 / Exact-Tree Assurance and Durable Evidence.** Run required canonical full local validation, exact production replay, no-std, benchmark, TryRuntime/integrity, embedding, runtime/client and dependency/threat checks. Confirm diagnostic traces against the uninstrumented production path where identity may differ. Preserve decision-bearing raw samples, trace/model inputs and prototype patches; qualify unavailable old bytes. After fixes refresh affected proofs and final-head CI/review evidence, not old-head checkmarks. **Exit:** The final reviewed source and artifacts satisfy the release assurance contract; historical or diagnostic-only passes are not promoted to current acceptance.

- [ ] **N7.4 / Public Truth and Guarded Publication.** Reconcile specs, architecture, integration, EN/RU Wiki, client authoring/status and release history. Explain the exact difference between old global ticket order and new persistent service order, between wake hint and executable condition, and between parked state and reclaimable state. State timing/load/provider assumptions, measured gains, retained retries, cleanup limits and lost historical causes. Follow current PR/release rules and obtain explicit publication authorization. **Exit:** v0.7.27 describes and publishes the actual reviewed model, with no claimed universal speed, perfect wake coverage on unsupported hosts or fixed latency under arbitrary load.

## 5. Minimum Witness Matrix

Rows below are acceptance coverage, not a mandatory one-EXP-per-test list. Reuse fixtures and parameterized/model tests when they prove the same claim. Build legal states through supported transitions, or establish an explicit construction equivalence before measuring a synthetic fixture.

### Execution, recurrence and safety

| Witness | Required observation |
| --- | --- |
| Start condition becomes false after Step 0 | The existing Cycle still follows Step-local conditions; no accidental idle parking or restart. |
| Top-up, fee/hold change or oracle update between Steps/retries | Fresh typed available amount and observation; no repeated committed prefix or retry reset. |
| True condition after completion | Declared recurrence and minimum interval; no same-block second Cycle. |
| Transient rise/fall or many changes while busy | Missed transient allowed; no fabricated historical crossing or next-Cycle latch. |
| False/zero/unavailable/invalid input | Declared distinct outcomes; unknown observation does not prove permanent uselessness. |
| No block Weight / insufficient service budget | Bounded paid prefix, no unadmitted effect, exact deferral/counter behavior. |
| Retired/parked sovereign receives assets | Custody unchanged; only permitted wake/reactivation follows. |
| Supported generic host/feature profile | Required semantics/coverage hold, otherwise admission rejects or selects documented timed review. |

### Ring order and membership mutation

| Witness | Required observation |
| --- | --- |
| Empty and singleton ring, large remaining budget | Empty terminates; singleton never executes twice by wrapping. |
| Partial round stopped/resumed across Actor phases | One round and independent Q1 guard survive; no frontier recapture. |
| Head/tail/interior cancellation during a partially served round | Surviving original members keep promised order; removed members' turns do not transfer to newcomers. |
| Ring `[A,B,C]`: A served, B/C removed, new member added | Neither A nor new membership gets an extra current-round turn merely because saved length was three. |
| Remove/reinsert or new Contract generation in the same block | Same-block protection survives; stale membership cannot authorize new-generation execution. |
| Many joins while original members remain unserved | New admissions cannot steal the current frontier or starve old eligible members. |
| Eligible head lacks only remaining Weight | Head priority preserved under the adopted contract; no cheap/privileged follower bypass. |
| Head proves no present service before parking | Paid transfer is allowed only by the specified semantic rule, not a disguised resource bypass. |
| Owner mutation of an arbitrary member | Bounded removal/repair, no whole-ring predecessor scan, valid round state. |
| Resident next Step / adjacent-round retry | No successor enqueue/rematerialize merely for one-block progression; execution metadata remains correct. |

### Parking, notification and capacity

| Witness | Required observation |
| --- | --- |
| Unrelated asset/feed changes | No unjustified Actor activation or full registry scan. |
| Relevant balance/spendability/provider/time boundary | Complete-domain certificate invalidates, or the declared timed fallback discovers persistent eligibility. |
| Update before/after registration and during negative check/acknowledgment | A newer change remains pending; no clear-dirty lost wakeup. |
| Repeated updates while pending | One logical activation-check obligation; no per-cause history or duplicate ring membership. |
| Shared-feed traversal interrupted by another revision or subscriber change | No skipped promised member; bounded restart/resumption without unbounded fanout. |
| Notification arrives while Actor is Running/retrying/paused/retired | No future Cycle, retry reset or unauthorized revival. |
| Pending/live/sleep destination full | Typed backpressure retains one durable obligation; no lost wake or unrelated worker poison. |
| Certificate becomes stale after Contract/runtime/config update | Reject/recertify/review under a paid bounded path; never indefinite stale parking. |
| Persistently false state with unchanged relevant dependencies | No repeated negative checks outside declared timed fallback; no pointless registration rewrite. |
| Repeated Park/Live/Sleep transitions near a time/condition boundary | Counted movement and negative work remain bounded; residency policy avoids provable needless oscillation. |
| Time wrap, overlapping wheel bucket or overdue backlog | No early retry, lost deadline, unbounded catch-up or hidden scan. |

### Storage, cleanup and physical evidence

| Witness | Required observation |
| --- | --- |
| Canonical state during Live/Park/Sleep transfer | Contract/custody not copied; single process/residence authority and generation-safe references. |
| Rollback after tentative page/cache changes | No abandoned in-memory or index authority survives; next Attempt reads canonical state. |
| Cached/compiled plan across relevant mutation | Freshness invalidated or recomputed; no stale condition authorizes spending. |
| Bitmap/membership summary disagrees with cell | Typed integrity failure/recovery; not silent disappearance or double execution. |
| Old generation partially swept while owner updates | New program, account and replay identity preserved; old cleanup cannot resurrect or remove new state. |
| Full retirement reserve / prolonged churn | Required revocation remains bounded; state and cleanup debt have explicit enforced limits. |
| Deep/max-population versus ordinary fixture | Resource model covers reachable worst cases, not just the successful shallow path. |
| Traces versus exact production replay | Diagnostic counters/resources are scoped and linked; end-to-end effects, order and state agree. |

A cold archived Contract still has a state cost; a bitmask still has update/proof cost; a generation still requires cleanup; a ring still needs admission and state persistence. No single representation substitutes for the above evidence.

## 6. Comparison Metrics and Release Gates

### Minimum measured report

Report useful outcomes and the work required to sustain them, using the fixed N6.1 populations/horizons:

| Domain | Required metrics / distinctions |
| --- | --- |
| Useful service | Successful effects, intended workflows/Cycles, distinct progressed Actors, committed Steps with their semantic meaning, detection/first-Step/completion latency, service gaps and censored/pending work. |
| Residency | Live/parked/sleeping/pending occupancy; useful turns per live visit; residency lengths; membership inserts/unlinks and successor publications per useful Step. |
| Parking | Notifications/source updates, relevant/irrelevant/coalesced checks, certificate refreshes, false wakes, registry writes, time-to-wake and timed fallback checks. |
| Movement | Live/Park/Sleep/Pending transitions, same-condition oscillations, capacity refusals, cancellation/repair cost and stale references traversed. |
| Resources | RefTime and ProofSize separately; requested versus outstanding maximum, actual settlement, logical accesses, distinct keys, encoded value bytes and recorded root-inclusive proof. |
| Lifecycle | Create/certify/update cost, retained bytes/holds, old generations, cleanup debt/rate and time to bounded recovery after an arrival burst. |
| Contention | Actor Control/effect/User use, head-resource refusals, negative-check budget, wake/maintenance share, sustained overload behavior and ordering changes. |

Do not invent a universal weighted score. A lower parked count or a shorter ring is not itself a win. A gain in live-Step rate that leaves wake discovery or cleanup indefinitely behind is not an end-to-end gain. Workload proportions matter; report where the design does not break even.

Sparse-population comparisons must acknowledge existing legacy due-frontier isolation. Establish newly removed negative-check, topology or publication cost instead of claiming that only the new scheduler scales with active work. Constant rounds/summaries or block baselines are separately attributable shared work, not an unbounded scan hidden in “fixed overhead.”

### Gate ownership

| Gate | Required closure |
| --- | --- |
| **Contract gate — N0** | Exact new states, ring ordering, Q1, block frontier, recurrence, retry and parking/reclaim rights. |
| **Service gate — N2/N3/N4** | One current-state executor and complete bounded live/sleep/park/pending/reclaim operation. |
| **Safety/resource gate — N5** | Every reachable owner and transferred hazard closed on the selected representation. |
| **Performance/design gate — N6** | Predeclared useful result, total costs/tradeoffs and one accepted retained design; no cherry-picking or moved workload. |
| **Release gate — N7** | Exact artifact identities, independent review and final-tree assurance with accurate public claims and explicit publication authorization. |

These gates do not require serializing all development; their proofs develop with their owning paths. A newly found safety issue cannot be deferred as optional tuning even after the finite candidate set is exhausted.

## 7. Migration and Preservation of Previous Work

This is the only proposed active root backlog. The earlier planning files remain provenance, not parallel specifications.

| Prior task or decision | Current treatment |
| --- | --- |
| Original G0–G7 soundness/redesign plan | Relevant hazards, raw-evidence hygiene, sacrifice decisions and release checks retained. No universal requirement to repair a completely removed old engine. |
| Revised current-state N0–N7 | All IDs preserved. No future-Cycle history, live amounts, retries, autonomous discovery and evidence applicability remain. |
| Previous N2.4–N2.6 physical closure | Strengthened for stable process ownership and certified Step/Wake plans; no presumption that a new VM or global cache is required. |
| Previous N3.1–N3.5 generic service choice | Now implements the selected persistent-residency/parking design, with logical wake/activation separated from the chosen carrier. |
| **New N3.6** | Owns mutable block-round/frontier correctness and no per-Step membership republishing. |
| **New N3.7** | Owns dependency-keyed parking, complete/timed wake plans and lost-wakeup closure. |
| **New N3.8** | Owns atomic residence transfers, arbitrary removal and full-capacity preservation. |
| Previous N4 parking/cleanup | Retained; Parked is explicitly not reclaimable, generation isolation and capacity backpressure cover the new indexes. |
| Previous N5.4 physical gate | Adds exact ring, wake, transfer and whole-maintenance closure. |
| **New N6.5** | Owns proof of residence savings and anti-thrashing break-even, feeding final N6.3. |
| Previous N6.4 deferred FIFO decision | User-selected cyclic residency is now the design direction; exact difference from old global FIFO must be ratified in N0, not presented as automatic equivalence. |
| Previous N7 | Retained with stronger independent round, wake, pointer, capacity and state/currentness challenges. |
| Conversational singly linked ring sketch | Candidate only; arbitrary removal, singleton behavior, mutable-round safety and real storage costs decide implementation. |

Verified work may satisfy an unchanged subclaim. A previously completed task whose domain has changed is qualified, not silently rechecked or blindly restarted. Keep accepted historical observations intact and show only the new obligations on the active graph.

## 8. Deferred Work and Non-Goals

### Proposed 0.7.28 physical-efficiency portfolio — inactive here

After a sound viable 0.7.27 design exists, the next campaign may compare broader ring/page layouts, bitmap widths, cache scopes, timing-wheel/radix variants, data packing, key ordering/hashers, shared immutable programs and additional Step shapes. Admission changes or resource redistribution require their own domain and policy decision.

Do not deliberately leave obvious redundant reads/writes in the new kernel for a later optimization release. Remove locally proved waste when it lies inside scope and is fully covered. Conversely, “another encoding might be faster” does not authorize an endless present sweep. The installed design's required bounds and cold/full/failure coverage are never optional.

### Other retained programmes

| Programme | Disposition |
| --- | --- |
| Full Local Causal Introspection | Deferred. Its old causal vocabulary needs requalification; ordinary harness diagnostics here do not implement it. |
| Network Physiology | Deferred. Historical cost anatomy is not automatically a measurement of this model. |
| CB2 / CG and other old candidate queues | Historical conditional evidence, not automatically N6 candidates or release dependencies. |
| Universal obligation registry / secondary scheduler | Not introduced merely by naming a logical obligation or wake plan. Reuse one current service owner. |
| Whole-program atomic recipes / Q1 relaxation / cost-priority bypass | Not selected. Require a separate semantic decision rather than a carrier shortcut. |
| External keepers or intents | Excluded as a requirement for ordinary Actor progress. |
| Complete historical experiment migration | Not required; preserve old records and qualify consumed claims only. |
| Old-line remediation | Independent scope if needed. A defect eliminated in the new engine is not retroactively fixed in the old release. |
| `$BLDR` capital bridge, builder invoice settlement, User creation gate, vacant-slot custody claim and broader framework work | Preserve historical task contracts at [S10]; no automatic import into current-state service. |
| Broader emergency-breaker programme | Separate scope; actual reachable breaker/control/custody behavior still requires release coverage. |
| New signing/binary distribution platform | No invented gate. Essential durable evidence and exact production identity remain required. |

No deferred heading can absorb an unresolved supported-domain safety or coverage blocker.

## 9. Final Closure Statement

The final Synthesis must answer all of the following without adding raw measurement branches:

1. Which current-state capabilities remain, which historical causes were removed, and how are incompatible old Contracts rejected?
2. Do multi-block Steps, current percentages/all-available, same-cursor retries and committed prefixes work without external executors or future-Cycle latches?
3. What exact order does the ring promise, what differs from the old FIFO, and how is a single block-round enforced despite membership changes and multiple Actor passes?
4. Which common Steps remain resident; what state actually changes; what old publication work was removed rather than shifted?
5. What constitutes a valid park reason, which dependency domains have complete wake coverage and which have explicit timed review?
6. How are negative checks, duplicate source updates and pending checks coalesced without a lost wake or unbounded fanout?
7. Where is canonical Actor data stored, how do membership indexes reference it and how are arbitrary owner controls bounded?
8. What happens when any destination or cleanup structure is full, and why does exactly one service obligation survive refusal/rollback?
9. How do generation-safe mutation and asynchronous cleanup protect new state and custody while bounding holds and debt?
10. What paid owners cover selectors, rounds, transfers, cache freshness, full/deep structures, effects and cleanup?
11. Which historical claims were imported, which hazards transferred and which removed mechanisms were eliminated by proof?
12. What whole-service improvement was established, under which population/load/horizon, and where do wake/polling/movement/cleanup costs limit it?
13. Which timing/order/state/economic tradeoffs were accepted, and which follow-up optimizations are truly optional?
14. Do source, accepted semantics, tests, Weight/Wasm, clients, public documentation and reviewed release identities agree?

**Ordinary acceptance requires semantic closure, physical closure, supported-domain soundness, the predeclared useful complete-path result and final assurance.** A semantic-only or correctness-only checkpoint is a different disposition requiring explicit task-owner acceptance.

“All gaps closed” means all known supported-domain safety/resource gaps and decision-critical unknowns have reviewed scoped outcomes. It does not promise to prove every possible future theorem or turn every old experiment into mandatory work.

## Source Register and Handoff

This is a consolidation of the supplied planning files and the latest conversation. N0.1 verified refs, post-release progress and current implementation shape only; N0.2/N0.3 and N1 still own semantic decisions, supported-configuration closure and evidence applicability. Source links below are inherited historical provenance, not automatic authority over the new semantic mandate.

The latest live-ring proposal supplies the design direction. The following are explicitly added design-closure requirements, not source-derived measured findings: robust mutable-round accounting rather than a length-only loop; bounded arbitrary unlink rather than an assumed free singly-linked removal; exact lost-wakeup/acknowledgment rules; separation of parked nonterminal state from reclaimable generations; and whole-service validation of residence savings. None claims that a particular carrier has already passed a benchmark.

| Planning input | SHA-256 | Treatment |
| --- | --- | --- |
| `DEOS_BACKLOG_0.7.27.md` | `68c5c6d8f87da26e295d777c94fc42dee40f3fff8e06e69831e7d496fe58f9bf` | Original G-plan superseded; applicable hazards and deferred-programme provenance retained. |
| `DEOS_BACKLOG_0.7.27_REVISED.md` | `113c6a1ccb0f2dc26c11fb949ca18d6dc0b2d08021726648bcf406565372226b` | Current-state mandate and historical applicability retained; service architecture now explicit. |
| `DEOS_BACKLOG_0.7.27_FINAL.md` | `73f6f00528cdd71a1bf808f5e772a938a699df2f91cae8b78c7002e46f7f8976` | All 32 N-task IDs preserved; generic service plan replaced by the selected persistent-residency/parking closure, with four new tasks. |

All previous N0–N7 task IDs are retained. N0.1 is complete; historical implementation and evidence may satisfy only explicitly requalified subclaims, while every remaining checkbox is unverified work. Do not add the supplied planning artifacts to runtime/build dependencies or maintain a second active backlog.

[S1]: https://github.com/atmo-network/deos/releases/tag/v0.7.26
[S2]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/template/pallets/actors/docs/specification.en.md
[S3]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/.agents/skills/architecture-experiments/SKILL.md
[S4]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/.agents/skills/architecture-experiments/tracks/actors/EXP-0117.md
[S5]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/.agents/skills/architecture-experiments/tracks/actors/EXP-0118.md
[S6]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/.agents/skills/architecture-experiments/tracks/actors/experiments.md
[S7]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/.agents/skills/release-assurance/evidence/candidate-attestation.md
[S8]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/BACKLOG.md
[S9]: https://github.com/atmo-network/deos/blob/c16675edf03e3af0d6dcf3080a31c64b49765a5b/AGENTS.md
[S10]: https://github.com/atmo-network/deos/blob/e449d20fd57aff48945663d974d0d4e8f0faffeb/BACKLOG.md

**First actions:** N0.2/N0.3 fix the finite ring/current-state contract; N1 imports applicable hazards and N6.1 declares measurements. Build the current-balance multi-Step/retry slice while independently testing ring rounds and parked-wake handoff. Then choose one co-access-informed carrier and close its resources. Do not begin with a whole-corpus migration, a large actor scan disguised as parking or a catalogue of low-level tricks.

**Governing rule:** keep useful continuations resident; park only with a complete wake or timed-review contract; move membership rather than the Actor; coalesce checks rather than preserve causes; preserve exactly one obligation under every mutation; price all work; measure the complete service; stop after the selected design closes.
