# DEOS Backlog

> Open framework work only. Specifications own normative behavior; code and tests own implementation and regressions; generated artifacts own exact resource bindings; Experiment Records own decision evidence. `BACKLOG.md` alone owns release scope and remaining work. Remove completed tasks from the active backlog without deleting their evidence.
>
> Pre-`1.0`: no DEOS network launches before `1.0`. The `0.7.x` line is fresh-genesis. Published tags and reviewed history remain immutable. Breaking authoring, storage and API changes must be explicit and tested, never silent reinterpretations of existing Contracts.

## DEOS 0.7.27 — Current-State Actors, Persistent Live Ring and Certified Parking

**Planning status:** repository reality is reconciled on branch `0.7.27` through the current local checkpoint above published `v0.7.26`. The current-state specification and independent oracle are active; stable process, service-ring, deadline, round, dependency, Pending and review boundaries exist in mixed implementation states. Oracle dependency publication is connected to one production caller with generated composed Weight, while the canonical Actor service ring, scan worker, V1/V2 execution and legacy-authority removal remain incomplete. `Accepted` Experiment status freezes a decision, not implementation, production reachability, executable tests, generated resources or end-to-end evidence. Report those states separately; helper and commit counts are not progress evidence.

**Immediate cutover blockers:**

| Entry/path | Missing prerequisite | Owner/test | Resource binding | Exact connection condition |
| --- | --- | --- | --- | --- |
| Canonical service V1 | One supported create/activate/progress/retry/complete path under only `ActorProcesses` plus `ServiceNodes` authority | N2.4, N3.5–N3.6, N5.1; atomic empty/populated publication tests and the shared semantic/carrier round trace close initial carrier ownership plus partial-round/refusal order | Generated composed extrema now cover atomic process publication plus empty/populated ring insertion, and standalone extrema cover round begin/probe/admit; supported caller conversion, current Step/retry, completion and unlink remain unbound | Every edge is transactionally single-authority; legacy Ready/Waiting is unreachable for that profile |
| Oracle Park V2 | Weighted source scan probe/member/completion/fault, source-cursor service, Pending delivery/consumption and Live return | N3.3–N3.8; lost-wakeup and occupied-destination fixtures | Generated publication already applies; scan, Pending, negative-check and transfer owners remain absent | Fixed-target scan advances only after durable Pending/stale proof and completion removes or retains exact source membership atomically |
| Timed-review V2 | Canonical due extraction, Pending check and return using the retained deadline carrier | N3.3, N3.5, N3.8; generic balance review fixture | Generated deadline probe/extract/move, check and refusal owners | Generic balance remains timed-review and one authority survives every full/refused destination |
| Production hard cut | Complete supported caller conversion and compile-time deletion of legacy scheduling authority | N5.1/N5.3; supported-path and absence regressions | Regenerated complete Actors/runtime/consumer bindings | No Actor/profile is simultaneously owned by legacy placement and canonical process/ring state |

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

## N3 — Persistent Live Service, Indexed Parking and Bounded Wakeup

- [ ] **N3.1 / Autonomous Discovery and Activation Checks.** `EXP-0121` freezes the source inventory, one generation-bound Pending-check boundary and retained-overload contract; runtime realization remains open across N3.2–N3.8. Implement runtime-owned discovery using bounded notifications, current dependency/source state and timed fallback. Hints authorize checks, not effects. A parked invalidation creates one generation-bound pending check; verify current start applicability before live admission. Cheap check and admission may share a transaction when fully priced, but logical roles remain distinct. Use direct registered account lookup where appropriate and bounded shared-feed traversal; neither whole-population per-block scanning nor unbounded per-source fanout is an acceptable implicit fallback. **Exit:** Work is discovered without external executors; all negative and pending work is accounted, overload assumptions are explicit and promised checks are not dropped.

- [ ] **N3.2 / One Current Obligation and Residence Policy.** `EXP-0122` freezes the logical return matrix and exact semantic threshold: work due no later than B + 1 remains Live under round/Q1 guards, while a known deadline after B + 1 transfers once to Sleep; Park requires a complete certificate and Pending owns an unacknowledged current check. Implement this policy across continuation, retry, idle review and completion. Ignore future-start hints while a Cycle is open; they cannot reset or reserve a second Cycle. On completion retain Live for the next permitted level check, Sleep to a known cadence/review, Park from fresh complete negative evidence, or terminate one-shot authority; avoid Park -> immediate Live churn and old-style Q1-only successor wakes. **Exit:** Recurring policies, multi-Step Actors and retries advance with one service owner; no same-block restart loop, double Cycle or unnecessary per-Step topology migration.

- [ ] **N3.3 / Certified Parking and Wake Completeness.** `EXP-0123` freezes the logical Park Certificate: exact generation and accepted-plan binding, typed negative reason, conservative exact dependency keys/revisions, and every relevant time boundary. Event-driven Park is allowed only for a proved closed mutation domain; otherwise retain one deterministic timed-review Sleep obligation. Generic balance/spendability remains timed-review by default until the host certifies every affecting mutation route; observation/feed and authored temporal paths may use exact invalidation/deadline authority. Implement the certificate and distinguish parked idle policies from open retries and owner-disabled state. Notification invalidation makes a check owed; it does not assert execution applicability. **Exit:** No live work is hidden by a stale/partial certificate; false-now never implies deletion, and every supported nonterminal wait has the promised bounded-cost return path.

- [ ] **N3.4 / Negative Work, Fairness and Griefing Budget.** `EXP-0124` freezes one complete coalesced activation check as the negative-work atom. A User secures its generated Trigger-control maximum before Pending materialization; a System check consumes bounded Actor Control allocation; each later timed review needs fresh authority. Duplicate/busy hints create no fee, work, retry reset or future Cycle. Implement retained prepaid authority, bounded repair retries, stale cleanup and strict valid-head FIFO without cheap-head bypass. Keep repairable provider/fee/resource refusal distinct from invariant fault, and cover tiny-credit storms, toggling, always-false/true programs, expensive heads, stale prefixes and sustained mixed demand. **Exit:** No free unbounded polling/fanout, hidden priority or repeated unpaid prefix work; contention and liveness hold within declared capacity/provider assumptions.

- [ ] **N3.5 / One Concrete Ring/Sleep/Index Carrier.** Implement and validate the carrier selected by `EXP-0125`: stable Actor-generation process records, one actor-keyed intrusive doubly linked service ring for Live/Pending, retained C32 deadline pages plus paged min-heaps for Sleep/Park reviews, process-owned Park evidence, and generation-bound reverse handles. The compile-safe `ActorProcess`/`ProcessResidence`/`ActorRef` and inert `ServiceHeader`/`ServiceNode` shapes now exist without shadow storage; a pure ring oracle covers empty, singleton, interior, cursor, wrap, and stale-generation mutation. Cutover inventory confirms create/activate/deactivate/finalize are transactional but placement authority is distributed across Ready/Waiting/Unsignaled frame mutations: Ready and Waiting can compile to Service and Deadline, while Unsignaled cannot distinguish valid Parked, Disabled, or missing evidence. The inert process shape now separates process-owned `Serving`, cause/authority-typed `Disabled`, irreversible `Retired`, and explicit plan-bound Park evidence; it creates no storage or behavior. A pure storage-free compiler maps typed Ready/Waiting/Park/Disabled inputs into exact process state and rejects evidence-free `Unsignaled`; it deliberately cannot authorize storage publication. A storage-free adapter now accepts only a coherent real `ActorControlLocation`/`ActorControlCell`, derives Ready Idle+latched work as Pending and other Ready work as Live, preserves exact Waiting handles, rejects malformed cells, and still requires separately supplied typed Unsignaled evidence. A source-backed regression inventories every raw Ready/Waiting/Unsignaled storage mutation owner across `lib.rs`, `scheduler.rs`, and `execution.rs`, classifying its transaction boundary and required process transition; any unclassified owner fails the focused test. A pure storage-free planner now consumes that exhaustive obligation type plus the current coherent process and typed transition evidence; it preserves process identity, compiles typed publication/successors, permits only explicit disable/retire removal, and rejects detach-without-successor, obligation mismatches, malformed current state, and evidence-free Unsignaled transitions. The same exhaustive inventory now binds every owner to a concrete publish/preserve/replace/terminal/carrier planner intent, its mutation-owner or complete direct-caller atomic publication cohort, and success-or-rollback outcome; the witness corrected benchmark-only Ready removal to its actual function-owned transaction. Canonical `ActorProcesses` storage and one transaction-required publication helper now exist inertly: publication rejects any remaining legacy locator/cell, replacement requires the exact stored current process, planner failure writes nothing, and enclosing rollback removes a staged successor. Canonical `ServiceHeader`/`ServiceNodes` storage and transaction-local append/remove helpers are also inert: they require matching generation-bound Service process residence and no legacy authority, preserve empty/singleton/interior/cursor links, reject stale generations or local corruption, and roll back staged mutation. Generated standalone production-runtime weights now cover the populated append maximum, atomic process publication over empty/populated insertion, atomic service retirement over singleton/pair-cursor/interior unlink, plus round begin, eligible probe and eligible admission carrier extrema; the artifact guard requires every owner, while no benchmark pretends these inert boundaries are a complete production caller. Canonical `DeadlineHeaders`/`DeadlinePages`/`DeadlineHandles` now add inert retained C32 deadline ownership: transaction-local insert/remove/move requires exact generation-bound Deadline process residence, reuses fragmented slots, rejects full/invalid destinations, unlinks empty pages, and rolls back failed movement. Clock-local `DeadlineIndexPages`/`DeadlineIndexPositions`/`DeadlineIndexLen` now form the inert paged binary min-heaps over exactly the nonempty canonical Block/Tick buckets: deadline-member publication/removal transactionally creates, validates or repairs the exact heap entry while direct index boundaries reject legacy authority, missing/corrupt headers, stale inverse positions and capacity overflow; full C32 page boundaries, clustered/overdue ordering, exact repair and enclosing rollback are covered without touching production wakeup traversal. No production caller reaches these boundaries, so old control cells remain sole authority until one cohort transactionally converts every placement/movement/removal owner together; do not populate `ActorProcesses` from `Unsignaled` or retain two residence authorities. Preserve direct arbitrary unlink, destination-first or transactional movement, one occupancy authority, no allocation on retained Live progress, and complete Block/Tick/review coverage. Open a second N6.2 candidate only if implementation falsifies a recorded bound. Cover empty/singleton/interior/wrap/full rings, fragmented/full pages, clustered/overdue deadlines, stale generations, refusal, rollback, and corruption. **Exit:** The selected bounded storage/index design is implemented with structural and generated full/fragmented evidence; common Live progress allocates no membership and every control operation has bounded authority.

- [ ] **N3.6 / Block-Round Frontier and Persistent Ring Service.** Implement the `EXP-0126` round protocol across Prepass/Drain or their approved replacements: consensus block B is the immutable round identity; `ServiceHeader` owns one persistent next-encounter cursor; each node owns `eligible_from` and `last_considered`; process state independently owns Q1 `last_attempted`. Inert transaction-required round boundaries now initialize one immutable `round_block`, classify Empty/Closed/AlreadyAttempted/Eligible at the persistent cursor, reject stale generation and malformed future markers, stamp an admitted attempt into process-owned `last_attempted`, and defensively converge an already-attempted head without a second attempt; compile/publication transitions preserve Q1, ring admission derives `eligible_from = B + 1` and `last_considered = B`, and production Prepass/Drain remain unchanged. The independent oracle preserves its next-encounter position across partial rounds, distinguishes candidate observation from admitted turn/cursor movement, and leaves a resource-refused head unchanged across later passes and the next block. One shared storage-neutral trace now proves the actual carrier matches the oracle for cross-block continuation, wrap, and independent RefTime/ProofSize refusal without importing pointer layout; generation, mutation, Cycle-outcome and Q1 differential fixtures remain open. Generated production-runtime owners measure populated append (`35,549,000` ps / `6,086` bytes), atomic retirement across singleton (`34,851,000` / `3,948`), pair-cursor (`42,394,000` / `6,086`) and interior (`46,724,000` / `8,634`) unlink, round begin (`10,127,000` / `1,511`), eligible probe (`17,321,000` / `3,550`) and eligible admission (`22,629,000` / `3,550`) without connecting a second scheduler authority. Stamp every B admission/reentry as already considered and eligible from B + 1, stop at the first B marker, and never advance a valid unaffordable head. Prove zero/one/many members, interruption, wrap, head/cursor/interior/last removal, cancellation and generation replacement without initial length, removable sentinel, phase-local cursor or scanned served prefix. **Exit:** No duplicate or skipped eligible turn, donated/recaptured turn, same-block reentry exploit, ring-wrap loop or phase-reset double service; a resident next Step needs no successor rematerialization.

- [ ] **N3.7 / Dependency-Keyed Parking and Lost-Wakeup Protocol.** Implement the `EXP-0127` protocol across exact-key registries/direct mappings, reverse ownership and bounded dirty/Pending traversal. Inert `DependencyRevisions`, `PendingCheckOwners`, and exact source/Actor `DependencyRegistrations` now establish checked source revision/exhaustion state, one Actor-generation/plan-revision obligation, and acknowledged-revision handles without notification traversal or production authority. Transaction-required revision mutation advances monotonically, writes sticky exhaustion instead of wrapping, and rolls back with its enclosing transition; install is overlap-idempotent, while replace validates the successor before overwriting exact old authority and remove accepts only the exact current handle. Inert transaction-local scan boundaries now capture one fixed source revision and fixed append horizon, retain a scalar cursor, derive each encountered exact handle from source-owned retained C32 registration pages, atomically remove an exact stale slot or advance its matching generation/plan Pending registration acknowledgment through the fixed target before moving the cursor, hand completion directly to the newest retained revision/horizon, and fail closed without erasing cursor state on authority refusal, exhaustion, or rollback. Exact reverse positions keep replacement/removal in place, append across full pages, and retain a bounded free-position stack; holes are reused only outside active scans so fragmentation cannot retarget a cursor or create unbounded append growth. An inert event-complete publication boundary now atomically advances the source and retains exactly one scan: it captures the new revision/horizon only when idle, otherwise coalesces behind the immutable active target, and preserves scan state on exhaustion or enclosing rollback. The finite production source-owner inventory is now closed to Oracle feed state: registration, pause/resume lifecycle mutation, deactivation, and changed or equal-value publication each bind the collision-free `OracleFeed(feed)` source schema, present transaction boundary, and exact post-write/pre-event publication point; source-backed runtime evidence rejects every unclassified `Feeds`/`Observations` writer. A transaction-required inert bijection now resolves each typed Oracle feed to one monotonically allocated scalar `DependencySourceId` with exact reverse ownership, idempotent reuse, sticky exhaustion, occupied-source refusal, and enclosing rollback; no hash or unbounded lookup can define source identity. The generic Oracle package now exposes one closed `OnFeedStateChanged` post-write/pre-event hook covering registration, pause, resume, deactivation, changed publication, and equal-value refresh under each owning transaction; exhaustive package evidence proves authoritative post-state visibility and rollback. The production DEOS adapter maps every hook cause through the exact feed/source bijection into the event-complete publication boundary in O(1), reuses identity, and fails transactionally on source/revision exhaustion; all six causes and enclosing rollback are covered. Oracle benchmark setup distinguishes allocation/begun registration from reuse/coalesced existing-feed lifecycle and publication paths, generated Oracle weights charge the composed Actors storage effects, and the existing changed-observation ingress remains unchanged. An inert Actors-owned exact circular source carrier now bounds occupancy by the maximum complete-plan registration population, preserves one fair cursor, admits only active scans transactionally, makes duplicate insertion idempotent, supports O(1) arbitrary removal only after scan completion, and fails closed on capacity/topology errors with rollback evidence. An inert combined publication boundary now retains even an empty fixed-horizon scan source, keeps exactly one membership across coalescing, and rolls revision plus carrier mutation back together. Production Oracle publication now resolves its exact source, advances the revision, and retains one fair scan-source membership in the same transaction; all six causes share this boundary, coalescing preserves one membership, and carrier-capacity refusal rolls back Oracle registration plus source allocation. Three distinct Actors benchmarks close empty-list begun, populated-list begun, and coalesced/already-active carrier insertion branches with generated production-runtime Weight ledgers; the benchmark artifact guard requires all three. Oracle-composed production weights now cover begun versus coalesced carrier effects across registration, lifecycle, changed publication, and equal refresh. Generated scan probe/member/completion/fault weights, explicit fault ownership, fair fourth-family shared-budget integration, and transactional completion/removal wiring remain ordered gates; the existing observation dirty list stays solely legacy feed-trigger fanout authority. Age expiry remains timed and incomplete balance/spendability remains timed. An inert single-source negative-evaluation commit now transactionally revalidates exact Actor generation/plan and source revision, installs or replaces one registration, acknowledges only that covered snapshot, and preserves old authority on refusal or rollback. An Actor-owned bounded complete-plan commit now prevalidates every desired source snapshot and exact retained registration, installs or replaces successors before removing obsolete sources, rejects duplicate/oversized/capacity-invalid plans, and preserves the complete prior set on refusal or rollback. The same transaction now retains one optional Actor-owned Block/Tick timed review, validates its exact generation/plan and strictly future clock domain before event mutation, and atomically installs, retains, replaces, or removes it while refusal and rollback preserve the complete prior event/time plan. An inert due-review boundary now revalidates the authoritative Block/Tick clock and exact current generation/plan/review, publishes one typed durable Pending destination before removing the deadline authority, and makes exact replay idempotent while early, stale, raced, occupied-destination, and rollback paths retain the prior owner. Exact Pending-review consumption now prevalidates the retained cause, atomically installs the complete event-only or event-plus-time successor plan before removing only that Pending authority, and preserves the prior obligation on replay, owner/source race, invalid deadline, capacity refusal, or rollback. Event-source scans now publish one exact Actor-generation/plan-bound source/revision cause into durable Pending event authority before acknowledging the registration or advancing the cursor; scan acknowledgment updates the matching complete-plan handle atomically, an already-Pending matching owner coalesces without cause history, and stale destination ownership, rollback, or source/owner races preserve the unacknowledged obligation. Exact Pending-event consumption revalidates the retained cause, owner, current source and acknowledged registration, installs the complete event-only or event-plus-time successor before removing only that event destination, and preserves authority on replay, mismatch, revision advancement, invalid deadline, capacity refusal, or rollback. Event and timed causes share one Actor-keyed logical Pending destination: either publication fails closed while the other cause is retained, and exact consumption releases only its cause before a later alternate publication can succeed. Keep each shared traversal target fixed, retain newer revisions as the next target, and advance a subscriber cursor only after stale proof or durable destination authority. Exhaustion is sticky and fail-closed; incomplete balance/spendability/provider domains retain timed review. Coalesce updates without cause history. **Exit:** Deterministic adversarial interleavings cannot lose the only future check, publish two activations or wake an unrelated/old generation; notification cost remains bounded and fully attributed.

- [ ] **N3.8 / Membership Transfers, Arbitrary Control and Saturation.** `EXP-0128` freezes one transactional residence exchange: preflight and stage the destination while the source remains authoritative, linearize once through `Process.residence`, then release the exact source and advance cursors/acknowledgment in the same commit. Normal full conditions retain the unchanged source with typed local backpressure. Pause/update/cancel/retire directly revoke and unlink without ordinary destination capacity; interrupted detach retains one generation-bound process-local cleanup cursor serviced only from mandatory Control reserve. Implement this across every Live/Sleep/Park/Pending edge, owner/protocol control, stale generation, last-member deletion, callback rollback and full carrier. **Exit:** Exactly one current residence, terminal cleanup authority or completed deletion remains after every outcome; no unbounded search, dropped invalidation or full-capacity terminal dead end, and every path has a complete resource owner.

## N4 — Disablement, Mutation and Asynchronous Reclamation

- [ ] **N4.1 / Revocation and Authorized Revival.** `EXP-0129` freezes one process-owned `Serving`/typed `Disabled`/irreversible `Retired` status protocol. Park remains automatic negative current-state evidence, never owner status. Revocation removes serving authority before bounded direct detach and preserves custody, committed Cycle prefixes, retry attempts/deadlines and failure state without economic rollback. Resume/reactivation is an explicit cause-authorized, generation/plan-bound current-state reconstruction with complete admission and B + 1 eligibility; it never reattaches a raw stale residence. Contract replacement retires the old semantic generation before an atomically admitted replacement can serve. Immutable/System rights do not expand implicitly, Retired has no revival edge, and ordinary wakes only detach stale references. Implement the finite status/Cycle/authority matrix. **Exit:** Cheap cessation of service and authorized mutation/revival are complete; nonterminal parked Actors are never mistaken for reclaimable objects.

- [ ] **N4.2 / Generation-Safe Owner and Protocol Cleanup.** `EXP-0130` freezes one sealed retired-generation cleanup manifest with fixed direct-residence, executable-auxiliary, detector/reverse-registration, semantic-state, resource-release and tombstone phases. Each generated unit deletes or proves stale one exact generation-tagged quantum and advances its canonical cursor atomically; no population scan, mutable discovered-work queue, semantic wake or generation-blind key deletion is allowed. Replacement generations use disjoint namespaces, while custody is excluded and holds/slots release only after prove-empty finalization. Mandatory maintenance is separately funded rather than idle-only, and admission must keep cleanup-debt creation below sustainable service. Implement the phase/interruption/replacement/full-capacity matrix. **Exit:** Owner mutation during sweep preserves the new program and custody; mandatory disablement remains possible at full capacity and cleanup progress is explicitly funded and bounded.

- [ ] **N4.3 / Retained State, Holds and Reclamation Debt.** `EXP-0131` freezes one generation-owned retained-byte hold plus admission-time terminal-capacity bond. Disabled state remains fully collateralized; retirement converts the presecured bond into exact classed cleanup debt without allocating at the mandatory edge. Component-wise payer/global high-water checks bound old generations and stale references; mutable admission backs off before saturation while mandatory revocation consumes reserved capacity. No post-seal old-generation reference may be created. Separately funded non-borrowable maintenance drains the canonical cursor after halt/interruption, and old hold/slot/bond release only after prove-empty finalization. Implement the lifecycle, saturation and recovery matrices and generate exact byte/debt/rate owners. **Exit:** No free cold storage, unsecured terminal debt, future-generation subsidy, custody mutation or silent transfer between storage collateral and block computation.

## N5 — Remove Obsolete Duties and Close the Physical Resource Model

- [ ] **N5.1 / Old-Path Reachability and Removal.** `EXP-0132` freezes a fresh-genesis hard cut: delete the old `pending_signal` Cycle latch, Opening snapshot/surfaces, Ready tickets/pages, waiting primary cells, per-Step successor publication and unmetered fallback; replace temporal/run/API forms only under one stable generation-bound process and Live/Sleep/Park/Pending residence machine. Preserve sovereign custody, provider anchors, fee collection and effect adapters strictly as non-scheduling boundaries. Implement every Manual, ingress, observation/Crossing, temporal, retry, lifecycle, genesis, independent-host and client route without a compatibility reader, dual write, migration or fallback scheduler; regenerate metadata, ABI and Weight owners atomically. **Exit:** Compile-time absence and supported-path tests prove deleted authority unreachable, preserved boundaries cannot schedule work, custody/effects are unchanged, and only one engine owns each behavior.

- [ ] **N5.2 / Complete Weight Owners and Reachable State Domain.** `EXP-0133` freezes one generated resource-domain matrix for create/certify, notifications, observation and negative checks, ring rounds, current execution/retry, arbitrary unlink, residence transfers, Sleep extraction, Park refresh, lifecycle mutation, cleanup, mandatory service and bounded APIs. Every descriptor admits prerequisite reads, selector CPU, distinct-key/root-inclusive proof, staged writes and mandatory refusal/rollback suffix before mutation. Task effects reserve then settle independently; dispatch Control, retained-byte holds and cleanup debt remain separate. Implement maximum/singleton/full/fragmented/deep, cold/hit, stale, refused and rollback fixtures with setup assertions, no assumed overlap discount and no fallback Weight. **Exit:** Every shipped route and retained class maps to generated owners over its complete legal domain, and production bindings reconcile requested/outstanding/settled Weight, physical keys/proofs and collateral maxima.

- [ ] **N5.3 / Known Hazard and New-Model Soundness Closure.** `EXP-0134` freezes the composed supported-domain matrix: selector and executor share one typed current-generation domain; callback-visible authority is revalidated; acknowledgments cover only evaluated revisions; block rounds remain immutable; residence transfers linearize once; custody survives scheduling refusal; mandatory capacity is presecured; and event-only Park requires a host-certified closed mutation domain, otherwise deterministic timed review or explicit rejection. Implement its exact reference/independent-host, callback, wake, round, transfer, lifecycle, cleanup, capacity and compile-time old-path falsifiers with EXP-0133 resource owners. **Exit:** Zero unresolved known supported-domain safety/resource blockers, with exact reviewed executable outcomes rather than a claim that all imaginable future defects were disproved.

- [ ] **N5.4 / Physical Architecture Closure Gate.** `EXP-0135` freezes the compact physical design: one stable generation process; one actor-keyed intrusive Live/Pending ring; retained C32 deadline pages and paged min-heaps; revisioned Park/Pending wake plans; immutable block rounds; transactional residence; sealed cleanup; and separate Control, effect, settlement, collateral and debt owners. It records exact selected encodings and structurally eliminated old successor-publication operations without claiming a measured benefit. Complete the remaining exit gates through N6.5 residency/churn evidence and runtime/model/property/generated proof; the Synthesis owns no raw benchmarks. **Exit:** One production-capable model has measured ordinary, negative, delayed, burst and maintenance costs; no essential round, wake, capacity or Weight proof is deferred to 0.7.28.

## N6 — Bounded Comparison, Residency Evidence and Final Decision

- [ ] **N6.1 / Economic Goals, Workloads and Materiality Freeze.** `EXP-0136` freezes twelve bounded workloads before current-lineage measurement: reference Fee Sink/Burn/liquidity and representative User goals; current-balance multi-Step runs; funded retry; recurrence; true/false churn; 10,000-identity sparse Park; paged wake storms; Sleep/reentry; owner mutation; heavy effects; blocked heads; and cleanup under demand. Exact populations, horizons, demand modes, outcome/censoring classes, equal-semantics versus H/L bridge boundaries, matched-noise rules and mechanism-sized materiality are fixed. Implement the named fixtures and instrumentation with N3–N6, without shrinking the matrix or moving its target after results. **Exit:** The finite matrix and executable fixtures detect moved polling/cleanup bills, lost work and order changes; no arbitrary universal rate or post-result target.

- [ ] **N6.2 / Minimal Necessary Physical Choices.** `EXP-0137` and its dated amendment retain one implementation baseline: stable process placement, actor-keyed intrusive Live/Pending ring, exact revisioned Park lookup, retained C32 deadline pages plus paged heaps, and sealed exact-handle reclamation. Route S requires correction of any evidenced boundedness, authority, wake, saturation/rollback or resource-soundness defect regardless of research allowance. Route P admits at most one performance-driven same-semantics alternative campaign only after a representative implementation misses a predeclared criterion or protected regression, complete applicable work identifies one causal physical owner, and one concrete alternative plus smallest falsifier are frozen; at most one bounded refinement is declared before measurement. Provisional Weight, stale coefficients and isolated large numbers do not qualify. If the baseline meets its criteria, stop; if the bounded alternative fails, record it and return the release decision without moving to another bottleneck. **Exit:** Every selected operation is executable over its complete legal domain, S remains mandatory, P is finite and evidence-qualified, and one implementation remains.

- [ ] **N6.3 / Final Whole-Service Production Comparison.** Compare selected new geometry against a sound same-contract reference where meaningful, and against H/L by the declared economic-goal bridge. Use exact final production-Wasm and uninstrumented confirmation. Report effects, intended workflow completion, delays, service gaps, censored/pending work, notifications/negative checks, retries, User/effect/Control Weight, state and cleanup debt. Include N6.5 churn and round costs. Validate any fixed-trace sensitivity arithmetic; do not sum overlapping local improvements or treat charged counters as independent physical bounds. **Exit:** Declared materiality and protected regressions hold on the whole retained service, or the finite campaign records its exact obstruction without a false speedup or indefinite extension.

- [ ] **N6.4 / Design, Sacrifice and Release Freeze.** Publish the composed accepted decision: what Actor means, ring-order/Q1 boundaries, live versus delayed/parked residence, current reads, preserved retries, lost transient/provenance guarantees, recurrence, billing and generation-safe cleanup. Explain physical choice and the measured price of moving work. Obtain approval for residual changed guarantees beyond §1.2. Freeze one engine; transfer only a compact measured residual ledger to the later optimization portfolio. **Exit:** The semantic and physical decision is complete, useful performance is established, and no unselected alternative becomes an automatic new 0.7.27 campaign.

- [ ] **N6.5 / Residency Benefit and Parking-Thrash Accounting.** Measure resident membership writes, round/header/guard cost, Step state persistence, park/sleep/live transfers, certificate and subscription churn, false wakes, wake latency, inactive-set traversal and sweeper debt. Compare long useful runs, repeated true/false conditions, bursty updates while pending, adjacent-block retries, long sleeps, interior cancellations and huge idle sets with a small live frontier. Track physical operations, not just status changes. The first operation ledger is V1 resident execution and reports useful effects, completed workflows, committed Steps, retries, latency, remaining work, membership allocation/unlink, successor publication, round/header/Q1 writes, logical keys/bytes, component-wise maximum/reservation/settlement and retained cleanup debt. V2 extends the same ledger through negative check, Park, source or timed review, coalesced Pending and Live return, including publication, scans, false wakes, revision churn and transfer refusal. These are scoped early witnesses, not the EXP-0136 matrix or sweeper closure. An instrumentation-only legacy reconstruction is not a claimed current benchmark. **Exit:** Stable residency demonstrably removes the targeted republishing cost without adding a larger ring/polling/index bill; parking wins or its precise break-even limits are declared, and ordinary Step accounting is never reduced to a fictitious one-header-write total.

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
