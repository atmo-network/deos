# DEOS Backlog

> Open framework work only. Specifications own normative behavior; code and tests own implementation and regressions; generated artifacts own resource bindings; Experiment Records own decision evidence. `BACKLOG.md` owns remaining work, priorities and release gates. Remove completed work from the active list without deleting its evidence. Do not grow a chronological implementation diary inside a task.
>
> Pre-`1.0`: no DEOS network launches before `1.0`. The `0.7.x` line is fresh-genesis. Published tags and historical observations remain immutable. Breaking authoring, storage, fee and scheduling changes must be explicit. This plan authorizes continuation of `0.7.27`, not publication, force-push, tag replacement or a second permanent execution engine.

## DEOS 0.7.27 — First Canonical Resident Actor and Parked-Balance Activation

**Planning basis:** branch `0.7.27`, reconciled at commit `163775acd1615897a21d5a9126a9169ff398ab5c`, tree `5d8410c1c3005d1ad5594da8dcdd9eeb727ffb74`. The Actors source subtree is unchanged from the inspected implementation checkpoint `6d44e92d75e7334b5763a6684c62e5279c39725f`; the intervening commit replaced only this backlog with the task owner's parked-balance clarification. The published historical baseline is `v0.7.26` at `c16675edf03e3af0d6dcf3080a31c64b49765a5b`. [R1–R3]

**Replacement scope:** replace the active `0.7.27` backlog with this continuation plan. Retain existing N0–N7 identities and their applicable evidence; add only **N3.9 — Parked-Balance Mode Selection and Delivery**. An unchecked item below names remaining, changed or integration work, not a request to repeat its already proved subclaims. No historical experiment is renumbered or bulk-invalidated.

**Primary near-term outcome:** one ordinary User Actor, created and funded through supported public runtime paths, actually executes a multi-Step Contract through the canonical persistent service ring, retains membership across adjacent-round Steps and retry, reads fresh balances, preserves committed effects, and uses no legacy placement authority.

**Next outcome:** a recurring Actor captures its final balance, parks, ignores subthreshold changes, and wakes through a bounded runtime-owned path only after a qualifying parked-period change. The default candidate observes net balance movement from the fixed parking baseline with a floor of **at least 100 times the watched asset's existential/minimum balance**. If that trigger cannot meet its correctness, coverage and cost contract, select the explicitly scoped alternative of verified incoming credits to parked Actors, including a source whitelist. Do not substitute generic polling-on-any-spendability-change for the requested trigger.

**Release outcome:** finish the supported current-state/ring/wake/lifecycle machine; close every known applicable correctness and resource blocker; remove legacy service authority; bind and measure the actual implementation. A first working Actor is the next decisive milestone, not the whole release.

---

## 0. Current Reality and Immediate Blockers

### 0.1 Reuse what exists; do not count it as a complete engine

| Surface | Current evidence classification | Required next connection |
| --- | --- | --- |
| Current-state specification and independent oracle | **Decision accepted; behavior tested in its existing scope.** Amounts are `Fixed` / `Percent`; the parked-balance amendment now fixes total-owned absolute net movement, an inclusive per-asset `max(authored minimum, 100 × minimum balance)` threshold, atomic arm/rearm, parked-only recurrence, revision-safe negative checks, and one conditional certified-credit fallback. The partial-round insertion policy is not yet incorporated. | Make the independent model distinguish the selected partial-round order and add the ratified parked-balance transitions. |
| Semantic reads | **Implemented, production-reachable and source-guarded; only the read side is resource-bound.** `load_actor_semantic_state` unifies lifecycle, service, observation and execution entry boundaries, but reads legacy placement as the production authority. | Convert every semantic writer and its complete resource composition atomically; a partial writer would create dual authority. |
| Process and service carrier | **Implemented and locally behavior-tested; not integrated or production-reachable.** Canonical process/ring storage, transaction-local helpers and several generated local carrier owners exist. | Route supported public creation, activation, actual Steps/retries and completion through these owners. |
| Deadline/Park/Pending mechanisms | **Decided and partially implemented/tested/resource-bound; not integrated end to end.** Bounded types, storage boundaries and local witnesses exist in mixed states. | Complete paid due/scan/check/transfer consumers and real round trips. |
| Oracle publication | **Implemented, production-reachable and generated at the producer boundary; not integrated with a complete consumer.** A post-state hook publishes dependency revisions and retains source-scan membership. | Drain through the real worker or coherently gate the producer before shipping. |
| Lifecycle and cleanup | **Decision accepted and partially implemented; not production-complete or completely resource-bound.** | Execute revocation, replacement, bounded cleanup and resource release on canonical owners. |
| Performance | **Workloads decided; standalone carriers measured; whole-service result absent.** EXP-0136 freezes W1–W12, but no canonical V1/V2 production comparison exists. | Instrument V1/V2 and bind the final composed implementation before any speedup claim. |

This table reconciles repository reality at the planning basis above; focused tests cover the later normative round/oracle amendment without upgrading unchanged implementation claims. `Accepted` remains decision evidence only. The active critical path is V1's single atomic semantic-writer/process/ring cutover; the exact immediate blocker is that every supported Publish/Replace/Remove caller and its full Weight owner must switch together before legacy placement can be removed. [R2–R8]

### 0.2 Blocking obligations

| ID | Exact blocker | Closure owner | Required exit evidence |
| --- | --- | --- | --- |
| B1 | Legacy placement still mirrors active identity/hot/admission and remains scheduler authority; removing it before process/ring publication and consumer cutover would lose the only service path. | N2.4, N3.5, N5.1 | All supported semantic/process Publish/Replace/Remove callers and their complete resource owners switch coherently; no dual authority or legacy fallback. |
| B3 | Ring helpers are not a full ordinary Actor service path; standalone generated values exclude remaining lifecycle/Step composition. | N2.1–N2.4, N3.1–N3.6, N5.2 | V1 through ordinary dispatch and the actual mandatory Actor service phase, with complete admission and settlement. |
| B4 | The ratified parked-balance contract is not yet implemented through a bounded complete source/review path. | N2.5, N3.9 | One bounded complete source/review implementation of the fixed final baseline and `100 × ED` floor; no busy tracking. |
| B5 | Net balance cannot identify real incoming credits or whitelisted senders. | N3.9 | Conditional verified-credit implementation if the default mode fails its frozen criteria; never infer provenance from net balance. |
| B6 | Oracle producers are connected ahead of scan/check/service consumers. | N3.1, N3.4, N3.7 | Bounded publication-to-consumption round trip, or explicit coherent gating before any shipping state. |
| B7 | Generation-safe cleanup and its resource/debt limits are not yet fully executable. | N4.1–N4.3, N5.2–N5.3 | Mutation/sweep/capacity traces and measured bounded maintenance; no reliance on idle-only cleanup. |
| B8 | Retained heaps, new selectors and callbacks still carry deep/full/rollback coverage obligations. | N5.2–N5.3, N7.2 | Reachable-domain coverage and complete composed charging, or verified elimination of the path and coverage of its replacement. |

Do not turn these eight blockers into eight new programmes. Each needs its smallest connected closure. Preserve the existing `cancel_run` close-allowance correction and requalify it against the retained lifecycle rather than reopening its old omission by accident. [R9]

### 0.3 Work order

```text
Intake + narrow semantic/round correction + known safety triage
                         |
                         v
V1: public User creation -> canonical semantic owner -> actual ring service
    -> several committed Steps -> real retry -> completion
                         |
                         v
V2-B: completed recurring Actor -> fixed balance baseline -> Park
      -> qualified parked change -> one check -> Live -> new Cycle
                         |
              +----------+----------+
              |                     |
              v                     v
V2-O: existing Oracle          lifecycle/reclaim
producer -> consumer          and full supported cutover
              |                     |
              +----------+----------+
                         v
whole-service workloads -> final resource/artifact closure -> review -> release
```

V1 must not wait for universal balance notification coverage, a new Oracle framework, exhaustive cleanup tuning, an alternate heap or a history-wide experiment migration. Its supported lifecycle and failure suffixes must nevertheless be correct and funded. A new helper is progress only when it discharges a named blocker on this path.

The semantic-authority prerequisite of the first V1 cohort is complete: `ActorSemanticStates` now owns dormant identity or active Identity/Hot/admission, while physical control placement is independently derived and strictly cross-validated. `ActorProcesses` still owns only generation, attempt guard, status and residence, and no production caller publishes it or enters `ServiceNodes`; ordinary execution still drains the legacy paged FIFO. Therefore the next retained N2.1/N3.5 slice must connect process/ring publication and mandatory service together with its composed resource and Weight owners; do not invent a second semantic reader, weaken physical validation or claim direct-state fixtures as ordinary V1 closure.

The cutover inventory has one semantic writer cohort, not one storage declaration. Every row below is part of the first retained caller/resource boundary; helpers or tests that bypass these composition roots do not discharge it.

| Semantic transition | Supported composition roots | Generated Weight owner |
| --- | --- | --- |
| Publish dormant | Dormant user/System creation | `create_user_actor*`, `create_dormant_system_actor` |
| Publish active | Active user/System creation, including explicit owner-slot and sovereign-ID variants | `create_user_actor*`, `create_system_actor*`, Crossing creation branch where applicable |
| Replace dormant → active | `activate_actor` through `do_activate_actor` | `activate_actor` plus the applicable trigger-index branch |
| Replace active → active | Contract replacement; lifecycle, Trigger, run, retry, latch, wakeup and service mutations in dispatch, detector, prepass and mandatory service paths | Owning dispatch Weight plus the generated trigger/prepass/service unit that performs the mutation; `update_contract` and close-substitution composition are included |
| Replace active → dormant | `deactivate_actor` through cancellation, queue/wakeup cleanup and contract removal | `deactivate_actor` plus its complete cleanup suffix |
| Remove active | Owner close, sweep, expiry, completion, apoptosis and every terminal execution path converging on `finalize_actor_loaded_inner` | `close_dispatch_weight_upper` and the initiating dispatch/prepass/service owner |
| Remove dormant | Dormant close through `close_inactive_actor` | `close_dispatch_weight_upper` |

The semantic map, compare-and-replace writer, lifecycle callers, loader and try-state guards have crossed over, but active physical cells deliberately retain mirrored Identity/Hot/admission until the service carrier and generated resource owners replace that projection coherently. Effectful Step, zero-Step and opening/running StopCycle execution now select one typed carrier-neutral close-or-publish residence intent containing only authoritative state plus current-versus-cursor-zero resources. Initial activation, resumed publication and those execution transitions all enter one legacy physical publication boundary; only that adapter derives the temporary scheduler view and owns FIFO placement, while callers retain their explicit unsignaled restoration, scheduling continuation, scheduler-exhaustion closure, events, Weight and settlement semantics. The remaining executable cut is process/ring publication → mandatory service and all residence mutations → legacy scheduler retirement → physical-field removal → benchmarks and generated bindings. Intermediate helper-only states do not close N2.1 or N3.5.

The first atomic carrier cut has one implementation seam, not a publish-only precursor. Replace the `ActorReadyTail` prepass snapshot and `live_queue_head` discovery loop with `begin_service_round` plus bounded `consider_service_head` encounters; adapt mandatory service to consume the captured ring head; route its `NextResidence` result to process/ring retention, deadline transfer or retirement; and switch initial/resumed publication through the same transaction before deleting the FIFO path. The retained FIFO adapter now loads an active generation-bound canonical `ActorRef` and compiles the validated legacy Identity/Hot/admission plus temporary eligibility/resource fields into one `LoadedServiceEntry`; `service_live_queue_entry` no longer receives an `ActorControlCell`. This narrows the replacement seam without publishing a second authority, but queue discovery/consumption and those temporary fields remain legacy-owned, so changing publication alone would still create inert ring members and lose executable resource ownership. Generated Weight must cover ring probe/advance and every retained publication/removal suffix before the mandatory hook can advertise the carrier.

The cutover field map is now explicit. `ServiceNode::eligible_from` replaces FIFO `eligible_at`: initial publication and every reentry set B+1, while a retained adjacent-round continuation keeps its node and therefore needs no new eligibility timestamp. Identity/Hot/admission load from `ActorSemanticStates`; current execution resources derive from the admitted current Step, while zero-Step uses its generated completion envelope. A Weight, block-resource, fee-collection or invariant refusal leaves the same ring head, node, process and round frontier untouched. Only a committed service transaction may consume the encounter: retained Live/Pending work updates the process and advances to the captured successor without unlink/reinsert, longer deferral first secures Deadline residence then unlinks, and close first completes terminal cleanup then unlinks/retires. Ring advance may not precede any destination or terminal commit.

---

## 1. Continuation Contract

### 1.1 Preserved decisions

- Runtime-owned autonomy; no keepers, external intent executors or required bots.
- Bounded multi-block Contracts; at most one committed Step per Actor per block; earlier committed effects survive a later Step failure.
- Current predicates and exactly `Fixed(value)` / `Percent(perbill)` on valid Task surfaces. `Percent(100%)` means all current Available capacity, respecting current fees, ledger restrictions and protected minima.
- `AbortCycle`, `ContinueNextStep` and bounded `RetryLater`. Retry retains the same generation/Cycle/cursor and reads fresh dynamic inputs. Admission refusal is not an executed failure.
- One open Cycle and one logical current service obligation. No queued future Cycle, busy funding history or retry-counter reset caused by external hints.
- Persistent cyclic service, generation-bound membership and one block-round identity. No cost/class priority and no bypass of an eligible resource-blocked head.
- Parked is nonterminal; Paused/Disabled is separately authorized; Retired cannot be revived. Contract mutation and old-generation sweeping preserve custody and new-generation state.
- One canonical semantic source plus derived residence indexes. Parking does not move the Contract or custody into a second store.

The current-state section of the specification remains the starting owner; reconcile superseded later sections before release rather than asking implementers to interpret two competing contracts. [R3]

### 1.2 Partial-round ordering

The ratified rule is **append behind all current residents as viewed from the persistent next-encounter cursor**. The normative specification, independent list model and selected carrier's insertion-before-cursor behavior agree on this outcome.

Required distinguishing trace:

```text
initial order A -> B -> C; A receives an admitted turn; next cursor = B
D is admitted before the next block; no more turns occur in this block
next block expected order: B -> C -> A -> D
```

The inspected model's fixed-array `push_back` instead yields `B -> C -> D -> A`. Add this counterexample before changing either side. If the owning specification deliberately selects the latter policy, change the carrier accordingly and record that choice; the two cannot remain different. [R4–R5]

New or reentered memberships in B stay ineligible until B+1. Deletion cannot donate a turn. Reentry or generation replacement cannot reset the same-block guard. Resource refusal preserves the same candidate through a later pass and the next block. Do not use initial ring length, a removable sentinel, or production-wide membership copying as the proof.

### 1.3 Parked-balance monitoring is not busy-state tracking

Balance activation exists only for a configured, recurring, **idle parked** Actor. A timed review of an idle parked balance watch is logically still this parking obligation even if represented by the deadline carrier.

No balance-trigger work may create a future Cycle while the Actor is Live, Running, retrying or Sleeping for an open Cycle. Current Steps continue to read real balances normally. A balance change during a retry can make that same Step executable, but does not wake it ahead of its retry policy or create another Cycle.

A cheap producer-side lookup may be necessary to discover that no parked registration exists. This bounded lookup still has a resource owner. “No busy tracking” means no Actor-specific baseline updates, credit accumulation, membership changes, full activation evaluation or Trigger fee for the busy Actor; it does not falsely promise zero cost to the underlying ledger operation.

Owner-paused/disabled and retired generations ignore ordinary balance activation. Other explicitly authored activation modes retain their own rules; an internal review timer must not secretly bypass the balance threshold and become an unconditional periodic trigger.

---

## 2. Parked-Balance Trigger: Required Semantic Amendment

The following requirements implement the task owner's clarification and the ratified N0.2 choices. They freeze one default and one conditional fallback before decision measurements; do not keep additional undocumented interpretations in code.

### 2.1 Fixed final baseline

For each explicitly watched asset/surface `a`:

```text
B_anchor[a] = authoritative final watched balance
T[a]        >= 100 * ED[a]
```

Capture `B_anchor` after the last committed Step effect and actual fee settlement, after releasing transient Attempt reservations, and after every balance-affecting finalization operation covered by the transition. Capture only when the configured recurrence requires another balance-activated Cycle. Do not enumerate all sovereign assets. The watched set comes from a bounded authored/derived plan.

Bind the anchor to Actor id, semantic generation, plan/configuration identity, a monotone parking episode and the exact asset/surface. This is wake metadata, **not** an Opening snapshot, a funding accumulator or a basis for future Task amounts.

If a completed Actor remains Live for one final permitted turn before parking, retain the completion anchor. A qualifying change between completion and installation of the parked registration must remain detectable; do not silently replace the anchor with the later balance and absorb the change. Alternatively, complete baseline capture and park publication atomically in the completion transition when its full cost is admitted.

Initial activation of a newly created or explicitly reconfigured Contract has no preceding completed Cycle. Define one explicit initial arm operation using the committed initialization balance. It must not mistake the initial balance for an incoming credit or retroactively replay deposits made while no parked registration existed.

Routine polls, subthreshold notifications, duplicate hints and negative checks must not move `B_anchor`. A new anchor is normally established by completion of the next admitted Cycle, or by an explicit authorized contract/rearm transition. Record any exceptional reset policy and its economic consequence.

### 2.2 Exact observed quantity and threshold

**Ratified default:** use absolute net change of the declared sovereign asset total-owned balance:

```text
delta[a] = abs_diff(B_now[a], B_anchor[a])
qualified = delta[a] >= T[a]
T[a] = max(authored_min_delta[a], checked_mul(100, ED[a]))
```

This interprets “balance change” as movement in either direction and “at least 100 times ED” as an inclusive boundary. The task owner did not separately choose increase-only behavior or per-credit versus cumulative incoming thresholds; freeze those distinctions explicitly. Do not silently convert the default to positive credits.

`B_now` must be bound to one named, authoritative host balance surface. Prefer the actual asset balance rather than transient Actor fee availability for this trigger; decide whether the host's free or total-owned balance best implements that contract and document it. **Do not equate balance, total-owned balance and spendable Available.** Effect amount resolution continues to use current Available independently. A hold/freeze change without a watched balance change is not automatically a balance trigger; a separately promised spendability/observation/time wake retains its own coverage obligation. [R10]

Use `ED[a]` in the same asset units as `B_now[a]`: the host's native existential balance for native currency and the authoritative asset minimum for another asset. Never multiply native ED and compare it with an unrelated asset balance. No price oracle is needed to interpret the threshold unless a separate explicit value-based policy is approved.

Unknown, zero, unrepresentable or changed asset minima need an explicit bounded admission/recertification outcome. Reject unsupported zero-floor profiles or require an explicitly governed positive floor; never silently admit a dust-sized threshold. Use checked/widened arithmetic, no unsigned underflow, silent saturation or overflow-to-zero. Bind configuration identity and define requalification when the minimum changes. On-chain configuration must never admit a threshold below the applicable floor.

The floor is a **wake magnitude**, not a fee, burned amount, storage deposit, proof of sender expenditure or guaranteed economic anti-spam cost. A party might recover transferred funds through an authored policy. Negative work, callbacks, notices and oscillation still need resource bounds and a payer.

### 2.3 Net change is not gross incoming value

The default compares against the fixed anchor, not against the most recent sample and not against a sum of absolute movements.

Examples at `T = 100 * ED`:

| Parked-period history | Default net-balance interpretation |
| --- | --- |
| `+60 ED`, then `+40 ED`, with no offsetting change | Qualifies at `+100 ED`; the first small update must not reset the baseline. |
| `+60 ED`, then `-60 ED` before a sampled check | Net zero; no required activation. |
| `+100 ED`, then a return to the anchor before any permitted observation | A transient may be missed in a declared sampled-current-state profile; do not claim historical-event detection. |
| A qualifying observation already retained as Pending, then balance falls back | Keep the check obligation; revalidate current applicability before starting a Cycle. A stale positive does not authorize an effect. |
| External funding during Running/retry | The Step sees current funds; no new balance activation, credit history or next-Cycle promise is created. |

With several watched assets, the first implementation should use bounded exact independent thresholds, with a declared `any qualified surface` rule unless the Contract explicitly requests a different bounded condition. Do not add quantities across unrelated assets. Shared hints must not activate unregistered assets/accounts.

### 2.4 Consume qualifying checks without oscillation or lost growth

A qualifying change creates **one** generation/episode-bound activation check. Repeated notices coalesce. The full current start condition is checked at its authoritative boundary; before Cycle start, revalidate it again or prove the carried facts remain fresh.

The first implementation must choose and test one negative-check/rearm rule. Required properties:

- An unchanged already-evaluated balance must not continuously enqueue checks merely because it remains outside the anchor threshold.
- A failed check must not silently erase a later change or rebase the final-cycle anchor.
- A further material balance movement that can enable the policy, or a separately watched condition becoming applicable, must get the promised reevaluation.
- Any last-evaluated revision/watermark is distinct from `B_anchor`, bounded, and explicitly priced.
- A transient threshold followed by a negative start result does not admit a second Cycle or reset retry history.

Choose between bounded notification invalidation and paid timed reevaluation according to the declared profile. A blanket `last_checked = now` or `dirty = false` is not a lost-wakeup proof. Avoid adding a general historical event log to answer this one question.

### 2.5 Timing and completeness under the revised contract

Do not require invalidation on every change to every possible execution predicate merely to implement the parked-balance delta trigger. Prove completeness for the **chosen wake contract** and separately for any other authored wake obligations.

A notification-driven implementation needs coverage of every supported mutation that changes the watched quantity. This may include direct transfers, protocol/Actor credits, XCM, issuance/burn, slashing, reaping/recreation or another asset-specific path. The list is closed by the actual host, not by a generic claim that every credit uses the same helper. No raw storage write may bypass a promised notification path.

A bounded timed review may implement the **same fixed-anchor threshold** when hooks are incomplete. It can miss intermediate net states and adds latency and polling cost; those are declared properties, not silent equivalence to complete event delivery. It is acceptable only if it meets the frozen correctness and whole-cost criteria. It must not become whole-population per-block polling or reset the baseline on each review.

### 2.6 Conditional fallback: verified parked-period credits with whitelist

If fixed-anchor net tracking cannot provide useful, affordable and adequately reliable activation on the intended host, N3.9 selects verified incoming-credit activation. This alternative is expressly authorized by the task owner; it does **not** authorize restoring the old future-Cycle machine or `PercentageOfLastFunding`.

For the fallback:

1. Install a registration only for the idle parked generation/episode. A dormant, busy, retrying, disabled or retired Actor does not accumulate activation credits.
2. Observe positive committed credits through certified host paths, including declared internal and cross-chain cases. A current net-balance difference cannot establish their source.
3. Apply an exact asset and bounded source whitelist at that boundary. An absent source, spoofable payload or unknown upstream sender never satisfies a concrete whitelist entry. Declare whether identity means the immediate certified payer/producer, not an inferred original user through intermediaries.
4. Reject duplicate/replayed callbacks and rollback all tentative activation evidence with the economic transaction. A self-transfer, charge/refund pair or balance-neutral ledger rearrangement must not become invented funding.
5. Use a bounded, threshold-capped sum of verified permitted credits during the current parked episode, with the same `100 × ED[a]` floor. Per-credit thresholding is not selected.
6. Keep only the bounded eligibility accumulator/covered authority necessary for one coalesced check, never an unbounded credit history. No credits from an open Cycle leak into the next parked episode.
7. Evaluate current conditions and spend current Available at execution. The whitelist governs **what wakes the Actor**, not ownership or earmarking of every token already at its sovereign account. A restricted-spending policy would be a separate feature.

Whitelist fallback may be the only supported balance-activation mode for a host if net tracking fails the gate. Keep only the selected production mechanism unless distinct supported use cases justify both. If neither meets the declared contract, report the exact blocking choice to the task owner; do not ship a silent generic-polling substitute.

---

## 3. Delivery Milestones and Non-Negotiable Evidence

### V1 — First ordinary canonical resident Actor (top priority)

V1 is closed only when a production-shaped runtime fixture does all of the following:

- Creates an ordinary **User** Actor through the real supported public dispatch, not direct insertion of `ActorProcesses`, ring nodes or semantic records.
- Funds custody and service costs through supported ledger paths. Recipients and asset accounts satisfy their real minimum-balance rules; no failed Transfer is counted as useful completion.
- Uses the real mandatory Actor service entrypoint with normal block/inherent context and generated path owners; no second test executor or `Weight::MAX` shortcut substitutes for admission.
- Runs a bounded multi-Step Contract with current `Percent` inputs. A normal external balance/market change between Steps changes a later resolution as declared.
- Produces a genuine supported temporary failure and then recovery through authoritative runtime state. Retry preserves cursor/Cycle/attempt counts and committed prefix. A mock-only synthetic success/failure switch is not sufficient for this integration claim.
- Demonstrates at least one adjacent-round retry remaining resident and one successful interior-Step progression without node allocation, unlink/reinsert or successor publication. Long retry Sleep/return receives its own V1 extension before claiming that branch complete.
- Completes the intended economic workflow with exact effects, fee outcomes and Q1/order trace; all lingering residence and cleanup work is reported.
- Leaves no legacy locator/control-cell/Ready-ticket owner for the Actor and invokes no legacy scheduling fallback. The candidate build must not run a second scheduler beside it to complete the path.

Choose the smallest existing Task/market fixture that supplies a real typed temporary failure; preserve its canonical adapter. Manual authorization or a positive supported current start check may start V1. **V1 does not depend on solving the parked-balance design first.**

A narrowly supported candidate checkpoint may explicitly reject not-yet-connected configurations, but cannot claim them implemented or route them through the old engine. All promised release profiles still have to be connected before N7.

### V2-B — Parked-balance round trip

Extend the actual V1 machinery, not a separate demo:

```text
last successful Step and fees
-> fixed final B_anchor and valid parking episode
-> subthreshold changes do not activate
-> cumulative net change reaches the declared threshold
-> one bounded check
-> current condition admitted
-> Live from the permitted next round
-> successful new Cycle
-> new final baseline
```

Include a deposit while busy, initial arming, a below-threshold negative review, a later qualifying change, Pending saturation, completion-to-park interleaving and generation replacement. If verified-credit fallback is selected, replace only the signal qualification step and add allowed/disallowed/unknown-source cases.

### V2-O — Existing Oracle producer-to-consumer path

Close one actual supported Oracle feed through registration, unchanged-value freshness update, value/lifecycle change, source scan, Pending, current check and Live return. Include false results and time/age expiry. Improving other Oracle cases may proceed after V1/V2-B, but every attached production callback must already preserve boundedness, transactionality and durable work.

Do not attach a producer to an indefinitely undrained new source list. Either finish the necessary consumer or explicitly gate the new publication coherently while that path is unavailable. Capacity behavior must not silently lose checks or poison unrelated work; any intentional upstream refusal is named and tested.

### Final release closure

V1, V2-B and V2-O are scoped milestones. The remaining supported modes, full domain resource coverage, cleanup, client/API bindings, W1–W12 and final review still gate release. One passing vertical slice does not certify maximum population, general host completeness or full semantic conformance.

---

## N2 — Complete the Actual Current-State Execution Path

- [ ] **N2.1 / Minimal Canonical Continuation.** Execute as the continuation slice of the joint N2.1/N2.4/N3.5 atomic cutover. Connect the current Run/Cycle representation to ordinary new-engine execution only when create/activate publish the canonical semantic/process/ring authority and the same change binds their composed resources. Prove Idle/open-run consistency, exact generation/Contract binding, cursor advancement, completion, zero-Step distinction and one current obligation. Keep independent semantic tests already passed; fill the missing production entrypoints without weakening the pre-cutover absence guard or adding a temporary dual reader. **Exit:** the V1 ordinary Actor advances and completes without direct test-state construction or legacy placement.

- [ ] **N2.2 / Live Amount and Predicate Evaluation.** Retain the implemented current-only evaluator. Verify `Percent` and `Fixed` through V1 public execution, including balance changes between Attempts, fee-native reservations, protected minima, shares and non-native assets. Keep the parked balance anchor entirely outside Task amount resolution. Reject removed authoring forms rather than reinterpreting them; do not reintroduce Opening or LastFunding state. **Exit:** current economic results and boundary outcomes agree with the declared surfaces and independent expected arithmetic.

- [ ] **N2.3 / Error Policy and Retry Continuity.** Connect all retained error policies to canonical residence transitions. Adjacent-round retries retain the member; later retries move through the paid deadline path. Neither busy credits, Manual requests, source updates nor parking metadata reset attempts. Admission/resource refusal does not consume an execution attempt. Preserve prefix effects and exact local/global exhaustion semantics. **Exit:** V1 retry succeeds after genuine recovery without repeating earlier effects; long-delay, abort, continue and permanent-error paths have their own executable closures.

- [ ] **N2.4 / Loaded Transition Context and Economic Owners.** Continue the joint N2.1/N2.4/N3.5 atomic cutover from the implemented stable semantic and Contract-generation owner. Retain generation one for initial Contract publication, checked increment for every authorized replacement, last-generation continuity through dormancy, unchanged generation across ordinary Hot updates, and the active `ActorRef` loader. Publish that reference into every process, ring, deadline, park and cleanup carrier; never derive generation from cycle nonce, admission identity or placement. Finish the read/write cutover begun by `load_actor_semantic_state`. Inventory every semantic/process Publish/Replace/Remove owner and convert create, activate, update, deactivate, cancel, completion and terminal paths together with their resource composition. Derive cursor/resources from their selected owners rather than adding synchronized copies. Carry validated loaded authority until a relevant callback/mutation invalidates it. **Exit:** ordinary callers no longer recover semantic state from legacy placement; rollback restores the entire owned transition; every canonical carrier is bound to the current semantic generation; no copied authority, unowned selector or stale cached balance exists.

- [ ] **N2.5 / Certified Step and Wake Plans.** Reuse existing Contract/admission plans and add only the bounded information required for parked-balance activation: exact watched surfaces, mode, per-asset floor, direction, source capability and generation/config identity. Separate static plan, final parking baseline, acknowledged check metadata and current Step values. Do not require a general dependency solver or new VM. **Exit:** only configured surfaces are observed; the plan is not rebuilt on every Step and stale plans cannot authorize wakes/effects.

- [ ] **N2.6 / Co-Access and Stable State Geometry.** Confirm the selected stable semantic/process, sparse Run and ring layout against actual V1 reads/writes. Close the existing writer split before considering alternative layouts. Bind parking metadata to the stable generation, not to a mutable queue position. Track the actual cost of semantic record plus process plus ring node; “one stable owner” is not automatically one read. **Exit:** one implemented layout, no Contract/custody copying on transfers, and an operation ledger adequate to detect duplicated loads or writes.

## N3 — Connected Service, Parking and Wakeup

- [ ] **N3.1 / Autonomous Discovery and Activation Checks.** Connect the minimum paid positive current check needed for V1 and then the negative/return paths for V2. Use actual runtime callers and ordinary mandatory service; no test-only alternative executor. Coalesce one generation-bound check, retain it under refusal, and distinguish applicability from permission to inspect. **Exit:** supported work is discovered and processed without external executors; every producer has a bounded reachable consumer or is explicitly gated.

- [ ] **N3.2 / One Current Obligation and Residence Policy.** Implement Live/Sleep/Park/Pending transitions on the canonical process. A continuation due by the next permitted round remains resident; longer retry/deadline work Sleeps. An open Cycle never enters idle balance parking. Balance-mode completion captures the fixed anchor and ends busy monitoring before parking; no new Cycle is queued by its own effects. **Exit:** at every committed boundary exactly one current residence/terminal authority exists and no per-Step successor publication is needed for ordinary resident progress.

- [ ] **N3.3 / Certified Parking and Wake Completeness.** Requalify completeness against the selected trigger, not every hypothetical mutation of Available. For net-balance mode prove notifications for the named balance or declare bounded threshold-preserving review. For credit mode prove committed-credit and source coverage. Independently preserve authored time/Oracle validity obligations. Registration and final-baseline installation cannot miss a post-completion change. **Exit:** no busy tracking, indefinite stale park or silent switch from balance delta to generic predicate polling.

- [ ] **N3.4 / Negative Work, Fairness and Griefing Budget.** Price nonmatching, unparked, subthreshold, duplicate, negative, refused and cleanup paths as well as positive activation. Keep tiny updates on the smallest bounded lookup path; they must not decode the full Contract, append Pending work or charge another full activation fee. Preserve complete one-check paid authority where already selected, but review the actual payer and retention after the new threshold gate. Test refundable/circular deposits and repeated threshold oscillation: `100 × ED` is not a spam-proof fee. **Exit:** no free unbounded notices/polling, counter reset, starvation hidden by censoring or busy-state bookkeeping proportional to incoming events.

- [ ] **N3.5 / One Concrete Ring/Sleep/Index Carrier.** Complete the coherent semantic writer + process/ring cutover on the selected doubly linked carrier and retained C32 deadline substrate. In one supported candidate build, no Actor executes through a parallel old scheduler. Convert the lifecycle closure, conditional initial service publication and generated callers as one coherent change; inert helpers are not its exit. Early unconnected profiles must reject explicitly and cannot be advertised as delivered. **Exit:** public-created V1 actors use canonical semantic/process/ring authority; empty/singleton/interior/full/deep/fragmented and rollback paths are bounded and priced.

- [ ] **N3.6 / Block-Round Frontier and Persistent Ring Service.** Connect begin/probe/admit/advance to actual Prepass/Drain, sharing one immutable block-round identity. Preserve the ratified next-encounter order, Q1, non-consuming refusal and B+1 admission/reentry. Differentially compare mutable runtime traces to the corrected independent oracle, not just the existing fixed three-Actor trace. **Exit:** V1 Steps and adjacent retries retain membership; no duplicate, donated, skipped or recaptured turn; no production full-ring snapshot or unbounded wrap.

- [ ] **N3.7 / Dependency-Keyed Parking and Lost-Wakeup Protocol.** Finish the existing revision/scan/Pending consumers, using exact source/generation/plan/parking-episode authority. Bound scan target and append horizon; retain newer work; acknowledge only after durable destination or exact stale proof. Complete one current Oracle path, including equal-value refresh, lifecycle and age expiry. Avoid introducing unneeded per-credit source registries when actor-local certified hooks suffice. **Exit:** V2-O and source/Pending saturation cannot lose work, grow undrained sources, duplicate residence or revive old generations; unrelated source updates remain isolated and charged.

- [ ] **N3.8 / Membership Transfers, Arbitrary Control and Saturation.** Connect and prove arbitrary unlink, source-preserving refusal and atomic residence exchange. Secure a destination or bounded retained pending authority before removing the source; advance source/review cursors only after that commit. Mandatory revocation must remain possible at legal capacity. Include callbacks, partially served rounds, interior cancellation, replaced plans, event/time coincidence and exhausted identifiers. **Exit:** exactly one valid obligation survives success/refusal/rollback; full capacity is a defined backpressure case, not unexplained worker poisoning.

- [ ] **N3.9 / Parked-Balance Mode Selection and Delivery.** Implement and measure the smallest fixed-anchor candidate of §2 on the intended native and one supported non-native surface. Freeze hook/review coverage, cost/latency criteria, all boundary semantics and inclusion rules first. Complete V2-B with public credits, no busy bookkeeping and a floor of at least `100 × ED[a]`. If it fails, record the exact blocking reason and activate the single verified-credit/whitelist fallback; no repeated polling redesign loop. Keep only the accepted mechanism unless distinct supported profiles justify both. **Exit:** one trustworthy, affordable parked-balance activation product is fully integrated, or the exact unresolvable tradeoff is returned to the task owner before release; neither mode restores LastFunding amounts or a busy future-Cycle latch.

## N4 — Safe Revocation, Mutation and Asynchronous Cleanup

- [ ] **N4.1 / Revocation and Authorized Revival.** Connect Serving/Disabled/Retired semantics to ordinary controls. Parked may be mutated rather than deleted. Explicit replacement binds a new semantic generation and invalidates the old baseline/registrations atomically; resume of the same generation preserves the declared retry and anchor rules instead of guessing from absence. Ordinary credits cannot revive owner-disabled/retired state. **Exit:** control rights, committed prefix and custody survive replacement, pause/resume and saturation without old-generation execution.

- [ ] **N4.2 / Generation-Safe Owner and Protocol Cleanup.** Execute the selected sealed exact-handle reclamation plan in bounded quanta. Old Park anchors, credit accumulators if selected, event/time handles and Pending entries are included. Sweeps cannot delete a new generation or enumerate sovereign assets. Epoch/era timing can make work eligible, but cannot create an unbounded single-block purge. **Exit:** interrupted cleanup coexists with public replacement; all deleted keys are old-generation qualified; mandatory progress has a real budget and never depends solely on spare idle time.

- [ ] **N4.3 / Retained State, Holds and Reclamation Debt.** Bind actual retained-byte and terminal-capacity costs, including baseline surfaces, whitelist/credit metadata, reverse handles and outstanding generations. A collateral deposit does not reserve future CPU/PoV. Secure mandatory revocation capacity at admission; cap debt and apply backpressure before ordinary creation/replacement can outgrow maintenance. **Exit:** no free cold state, unbounded old-generation growth or premature hold/slot release; reported performance includes debt arrival, service and drain time.

## N5 — Removal and Complete Resource Closure

- [ ] **N5.1 / Old-Path Reachability and Removal.** Hard-cut the supported build from legacy placement, tickets, future-Cycle latches, historical Crossing duties, Opening snapshots and LastFunding accumulation. Preserve effect/custody/credit-authentication boundaries. A newly selected parked-credit adapter may reuse a verified producer boundary, but not its old scheduling authority. Inspect dispatch, genesis, hooks, simulation, recovery, embedding and feature builds; no compatibility reader or dormant fallback may recreate legacy service. **Exit:** V1/V2 and every promised release profile are served by one engine; absence assertions and ordinary-path tests agree.

- [ ] **N5.2 / Complete Weight Owners and Reachable State Domain.** Generate complete composed owners alongside caller conversion. Cover semantic publication/removal, ring rounds, actual Step/retry, all notice/check outcomes, final anchor capture, threshold arithmetic, whitelists/credits if selected, deadline extraction, scans, transfer refusal and cleanup. Reuse sound unchanged owners only with exact applicability. Include first/middle/last/full/fragmented/deep, cold/hit, selector/executor and rollback suffixes. Measure pure control flow and producer-side no-match work. **Exit:** every reachable production segment is owned; `actual <= reserved` is backed by a sound model, not two matching underestimates; no provisional Weight enters a release performance claim.

- [ ] **N5.3 / Known Hazard and New-Model Soundness Closure.** Close B1–B8 and all applicable inherited hazards with executable evidence or proven elimination. Add fixed-anchor drift, below-floor wakes, per-asset unit confusion, credit duplication/source spoofing, lost completion-to-park changes, stale episode acknowledgment and busy-state accumulation. Complete the round/generation differential suite and full-capacity recovery. **Exit:** no known supported-domain correctness/resource blocker remains; a design-level Accepted record or green source-string test alone cannot close a behavioral claim.

- [ ] **N5.4 / Physical Architecture Closure Gate.** Compose the actual retained semantic/process/ring/wake/deadline/cleanup implementation and its generated owners. Replace design-only closure labels with actual caller, test, binding and whole-operation evidence. Qualify physical choices if measured replacement costs contradict their purpose. **Exit:** one complete production-capable model; ordinary resident progress avoids the targeted republishing work, and no essential wake, capacity, cleanup or Weight proof is deferred to `0.7.28`.

## N6 — Useful Measurements and Finite Design Choice

- [ ] **N6.1 / Economic Goals, Workloads and Materiality Freeze.** Retain EXP-0136 W1–W12 where their premises survive. Before new decision runs, amend only the rows changed by parked-only `100 × ED` activation: recurrence, true/false churn, sparse Park, threshold/credit storms and balance review latency. Classify previously affordable subthreshold starts as intentionally outside the new promise, not missing successes. Preserve populations/horizons unless the accepted semantic change truly requires a documented revision. Freeze the V1/V2 fixture contracts before collecting their decision outputs. **Exit:** no post-result target adjustment; every sacrificed capability and cost transfer is visible.

- [ ] **N6.2 / Minimal Necessary Physical Choices.** Continue EXP-0137's selected baseline and its latest S/P amendment. Safety defects require correction regardless of experiment budget. At most one qualified performance-driven physical alternative campaign with one predeclared refinement is admitted after a complete representative slice identifies a material owner. The explicitly authorized net-balance versus parked-credit decision is a bounded semantic-mode choice under N3.9, not permission for endless carrier alternatives. **Exit:** one retained implementation; no ritual second prototype, arbitrary coefficient trigger or premature physical freeze against contradictory evidence.

- [ ] **N6.3 / Final Whole-Service Production Comparison.** Confirm final workloads on exact production Wasm with matched semantic/resource identities. Equal-new-semantics comparison is CS1/CS0 only; compare H/L separately by economic goal and declared changes. Include User fees, effects, rejection/censoring, notifications, review timers, residency, source lag and cleanup. Do not compare small deposits that intentionally no longer qualify as if the new engine had lost valid promised work. **Exit:** frozen useful materiality and protected limits hold, or a finite, honest negative result names the remaining decision; no fabricated universal speedup.

- [ ] **N6.4 / Design, Sacrifice and Release Freeze.** Record the chosen balance quantity, direction, floor, baseline lifetime, mode and whitelist semantics; recurrence and round order; retained retries; missed transient states; fees; and generation cleanup. Distinguish free/total balance detection from Available-based spending. Freeze one design only after V1/V2 and resource/evidence closure. **Exit:** reviewers can state what this Actor guarantees, what it no longer promises and why the measured tradeoff is acceptable.

- [ ] **N6.5 / Residency Benefit and Parking-Thrash Accounting.** Start instrumentation on V1, not another isolated helper. Count membership allocation/unlink, successor publication, semantic/process/node/header writes, input reads, retry transitions, effects, fees, time and remaining work. V2 adds final-baseline writes, per-credit no-match/subthreshold cost, negative checks, registration/acknowledgment changes, source scans, review timers and cleanup. Measure busy-credit storms and repeated unchanged reviews separately. **Exit:** saved republishing and avoided checks exceed their replacement costs on the claimed scope; no fictitious “one header write per Step” or free parked population.

## N7 — Bind, Independently Review and Publish

- [ ] **N7.1 / Final Cutover and Artifact Identity.** Bind the final source tree, benchmark Wasm, generated Actors and dependent Oracle/Router/effect weights, production Wasm, metadata, ABI/PAPI, bounds, cost/fee vectors and clients. Retain unchanged artifacts only with applicability evidence; incompatible authoring must be explicitly rejected/versioned. Ordinary public paths, independent embedding and all supported feature builds share the declared contract. **Exit:** one reproducible artifact set and no legacy/provisional numerical authority masquerading as final binding.

- [ ] **N7.2 / Independent Round, Wake and Resource Review.** Reviewers attack B2's distinguishing order, source/consumer saturation, watched-balance definition, baseline races, tiny-credit griefing, fallback source identity, callback rollback, full/deep heaps and old-generation sweep. Derive expected traces independently of implementation. Fix findings in their owners and rerun affected proofs, not the entire corpus by reflex. **Exit:** every material finding is resolved or removed from the enforced supported scope with explicit authorization; no claim relies solely on tests generated from the same algorithm.

- [ ] **N7.3 / Exact-Tree Assurance and Durable Evidence.** Run the repository's required full validation on the frozen final tree, plus the complete V1/V2 and supported-domain matrix. Retain decision-bearing raw outputs, commands, environment facts, benchmark and production identities, source manifests and regression seeds. Use the interactive-workstation reassessment protocol, not a quiet-host requirement. **Exit:** exact-tree assurance is reproducible, local versus external CI evidence is distinguished, and host noise is not used to excuse an unowned path.

- [ ] **N7.4 / Public Truth and Guarded Publication.** Reconcile specification, architecture, embedding, Wiki EN/RU, authoring and release notes. Explain the parked-only threshold and selected fallback without advertising it as a sender-spending restriction or universal notification guarantee. Publish measured benefits and limitations, unsupported forms and remaining noncritical research. Use the existing release procedure only after explicit task-owner authorization. **Exit:** tagged code, claims, accepted decisions and final evidence describe the same system.

---

## 4. Required Proof Matrix

Tests below are bounded witnesses, not mandatory new EXP IDs. Use the existing test/runtime infrastructure; retain current passing proofs only within their unchanged domain.

### 4.1 Balance, baseline and fallback

| Witness | Required result |
| --- | --- |
| Deposit during Step execution or retry | Current economic reads see it; no balance-trigger state/credit sum/next-Cycle promise changes. |
| Periodic Actor active between due points | Its own schedule remains authoritative; parked-balance notifications do not replace retry/cadence logic. |
| Last effect + actual fee + released fee reservation | Anchor equals the declared final authoritative balance, not a provisional value. |
| External credit between completion and Park installation | It is either included by the defined atomic snapshot boundary or remains visible relative to the retained completion anchor; never swallowed by a later reset. |
| Delta `T-1`, `T`, `T+1` | Inclusive qualification at T under the ratified default; exactly one owed check, not one per callback. |
| `+60 ED` then `+40 ED` with an intermediate negative review | Qualifies at the fixed-anchor total; review cannot drift the baseline. |
| `+60 ED` then `-60 ED` before sampled observation | No gross-volume qualification in net mode. |
| Decrease by T | Matches the explicitly frozen direction rule; no unsigned underflow. |
| Same observed balance after a failed start check | No perpetual duplicate activation; later material growth or another promised dependency change still gets its check. |
| Native and non-native assets | Correct minimum and units for each; no cross-asset sum or native-ED substitution. |
| Zero/unknown ED, threshold overflow or config revision | Explicit supported-domain refusal/requalification; no zero/dust gate or stale policy. |
| Hold/freeze/spendability change without watched-balance change | Does not pretend to be a deposit/balance movement; any independently promised wake remains covered. |
| Tiny unrelated credits and unregistered assets | Only the priced minimal producer path; no full Actor/Contract evaluation, baseline rewrite or Pending growth. |
| Pending already exists; newer change; destination full | Exactly one durable obligation survives; no blind acknowledgment or source loss. |
| Pause/retire/update/recreate before callback | No unauthorized revival; new generation has fresh declared arm state; old callbacks and sweep cannot affect it. |
| Fallback: allowed/disallowed/unknown source | Only certified whitelisted positive credits qualify; inference from transaction labels or net balance is forbidden. |
| Fallback: repeated callback, outer rollback, self-transfer, refund | No duplicate/counterfeit credit and no state retained from rollback; qualification follows the exact certified policy. |
| Fallback: several small permitted parked credits | Matches the selected aggregate/per-credit rule; no busy-period accumulation, unbounded history or LastFunding amount basis. |

### 4.2 Ring, continuation and ownership

| Witness | Required result |
| --- | --- |
| A served; cursor B; new D; next block | Exact ratified distinguishing order; model and carrier agree. |
| Empty/singleton/pair/interior/cursor removal and reinsertion | Bounded exact neighbor updates, no missing member or donated turn. |
| Resource refusal in each Weight component | Same eligible head retains priority across passes/blocks; no execution attempt counted. |
| Resident Step and adjacent retry | Same membership survives; current values are reloaded; cursor/attempt and prefix are correct. |
| Long retry Sleep/return | One obligation, no early service, no generation loss, no future Cycle. |
| Current start becomes false after first Step | The existing Cycle obeys Step conditions/error policies; it is not parked by reevaluating its original start gate. |
| Public create/update/deactivate/close | One semantic owner and one residence/terminal authority; no legacy fallback or direct-fixture-only success. |
| Atomic callback failure after tentative state/fee/index changes | Correct rollback of all owned state; the next Attempt cannot use abandoned cached authority. |
| Oracle update during scan/negative acknowledgment | Covered revision only is consumed; newer promised work and bounded backpressure persist. |
| New generation while old sweep runs | New code, custody, wake registration and same-block guards remain isolated. |

### 4.3 Resource and empirical distinctions

Keep separate: raw benchmark samples, fitted RefTime, generated ProofSize, logical accesses, distinct keys, recorded root-inclusive proof, requested/outstanding reservation, settled Weight, token fees, state holds and cleanup debt. A lower settled charge does not prove a smaller physical proof. An additional shared key may invalidate a claimed bound even if a simple coefficient comparison looks favorable.

Measure selection before mutation, failed preflight, producer callbacks and rollback suffixes. Reuse is allowed only within the recorded scope. No new selector is free because it has no DB reads. No old coefficient is sound merely because a new workstation run is noisy.

---

## 5. Evidence, Comparisons and Stopping

### 5.1 Existing results

- Preserve EXP-0120–0137 and earlier sealed observations. Add dated applicability amendments or a newly earned bounded claim only where this request changes meaning.
- Keep the existing correction for `cancel_run`; qualify its resource composition when lifecycle changes.
- The reported partial-round discrepancy is an open model/carrier consistency finding until the executable trace closes it. Do not describe it as an exploited released-runtime bug.
- Retained deadline heaps keep deep/full/fragmentation obligations. Dropping old Cadenced semantics alone does not remove them.
- The old due-frontier experiment already showed isolation from added non-due identities in its scope. Do not sell parking as though legacy execution necessarily scanned every Actor.

### 5.2 W1–W12 amendment discipline

EXP-0136 remains the starting twelve-workload contract, not a claimed set of completed current-engine runs. Amend the exact affected semantic premises before measurements. [R7]

Add parked-balance witnesses as explicit extensions to recurrence/churn/sparse/wake rows, not as a second independent mega-suite. A sparse event-Parked population must use a domain with actually proved notification coverage; do not manufacture 9,872 event-complete balance Actors if the host only supports timed review. Change a semantically impossible setup through an explicit pre-measurement amendment, preserving its economic purpose and publishing the distinction.

V1/V2 may establish scoped early results before the entire matrix. They cannot make a release claim without final binding and protected mixed-service/maintenance checks. Benchmark setup may create large fixtures through controlled helpers when clearly labelled, but the first ordinary-Actor integration witness must use normal public paths.

### 5.3 Success and finite fallback

- V1 success is real canonical execution with full resource ownership, not a throughput target.
- Net-balance mode success requires correct final-anchor semantics, supported coverage or explicit sampled review, bounded producer/check costs, declared acceptable latency and no busy-state activation work.
- If net mode fails, record the exact failure and run the one authorized verified-credit/whitelist alternative. Do not endlessly tune generic polling or expand all Oracle coverage before deciding this mode.
- Performance acceptance remains the predeclared whole-service materiality, with explicit tradeoffs. No mandatory `100 Actors/block`, `2×` or universal percentage is introduced.
- Safety correction never expires with a research allowance. Conversely, an attractive unrelated optimization does not extend the release automatically.
- If neither activation mode or the complete engine meets the agreed contract, return the exact unresolved decision. Do not silently weaken thresholds, drop unfinished Actors, count failed effects or publish design-only success.

### 5.4 Compact milestone report

For each V1/V2 handoff report only:

```text
exact commit/tree and runtime identities
public entrypoint and supported Actor profile
connected path and remaining blockers
actual executed tests and commands (not merely test source presence)
semantic/economic outcomes and chronological trace
complete resource owners plus remaining qualified domains
operation ledger, pending work and maintenance debt
next smallest connected deliverable
```

There is no additional report per helper. The next report should demonstrate a closed behavior, not another inventory of inert types.

---

## 6. Deferred Work and Retained Programmes

| Programme | Disposition |
| --- | --- |
| Broad page/fanout/bitmap/hash/key-layout/cache sweeps, shared immutable programs and speculative fast paths | Proposed `0.7.28` portfolio after one sound, measured current engine. Local redundant work can still be removed when equivalence and coverage are clear. |
| Timing-wheel/radix replacement by preference alone | Not active. The selected carrier gets one implementation; a bounded qualified S/P route can reopen it. |
| Full Local Causal Introspection / Network Physiology | Deferred; new diagnostics do not instantiate either programme. |
| General future-Cycle event history, LastFunding amount mode and a second scheduler | Excluded. The permitted parked-credit fallback is a narrow activation qualifier only. |
| Whole-program atomic recipes, Q1 relaxation and cost-priority bypass | Not selected; require separate semantic approval. |
| Universal event-complete balance/spendability interface for every host | Not required. Ship the explicitly supported net/review or verified-credit profile and do not overstate its guarantees. |
| Further Oracle feature expansion | Improve after the connected required paths; existing supported reachable callbacks still need full correctness and pricing. |
| Complete migration/renumbering of historical EXP files | Not required; qualify consumed claims only. |
| External keepers or intents | Excluded as a requirement for ordinary Actor progress. |
| `$BLDR` capital bridge, builder invoice settlement, User creation gate, vacant-slot custody claim, obligation identity and capacity-release scheduling | Preserve the historical task contracts and entry conditions; no automatic import into this release. [R11] |
| Broader emergency-breaker work | Separate scope, but actual reachable breaker/revocation/custody behavior remains in release coverage. |
| New signing/distribution platform | Not an invented release prerequisite; exact artifacts and essential durable evidence remain required. |

No deferred heading may absorb a supported-domain safety or resource blocker. Historical remediation for an old release is distinct from eliminating the path in this fresh-genesis release.

## 7. Final Release Statement

Before publication, the final Synthesis must answer:

1. Does an ordinary publicly created User Actor run entirely on the new semantic/process/ring owners, including retry and completion?
2. Which per-Step membership operations actually disappeared, and what operations replaced them?
3. What exact parked-balance quantity, direction, baseline boundary and `100 × ED` rule shipped?
4. Was fixed-anchor net mode retained, or was verified-credit/whitelist mode selected, and why?
5. How are busy periods, subthreshold changes, negative checks, late credits and configuration changes handled without lost wakes or baseline drift?
6. Are all attached Oracle/balance publishers matched by bounded paid consumers or explicit support gating?
7. Are Sleep, mutation, arbitrary unlink, full capacity and old-generation cleanup correct under the final resource model?
8. Which end-to-end economic result improved, under which changed guarantees, and with what latency, state and maintenance tradeoffs?
9. Which exact artifact/test/review identities support each of those claims?

The required conclusion is a working, measured machine with a precise activation product. It is not “all design experiments are Accepted.”

---

## Sources and Provenance

These links identify the inspected source basis, not files to overwrite. Read the working tree first. The task owner's latest parked-balance clarification supersedes conflicting planning premises, but does not assert that the new behavior already exists.

- **R1 — inspected branch/commit:** `0.7.27` at `6d44e92d75e7334b5763a6684c62e5279c39725f`, tree `7807ddb705ef885d9ee02d2c83714a412fb9e1f1`. [Commit](https://github.com/atmo-network/deos/commit/6d44e92d75e7334b5763a6684c62e5279c39725f).
- **R2 — checkpoint backlog and blocker map:** [BACKLOG.md at the inspected commit](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/BACKLOG.md).
- **R3 — current normative amendment:** [Actors specification §2.3 at the inspected commit](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/template/pallets/actors/docs/specification.en.md).
- **R4 — independent round oracle:** [current_state_semantic_oracle.rs](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/template/pallets/actors/tests/current_state_semantic_oracle.rs).
- **R5 — selected carrier and semantic loading code:** [actors/src/lib.rs](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/template/pallets/actors/src/lib.rs); [scheduler types](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/template/pallets/actors/src/types/scheduler.rs).
- **R6 — current lineage and hazard routes:** [Actors experiments index](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/.agents/skills/architecture-experiments/tracks/actors/experiments.md); [EXP-0135](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/.agents/skills/architecture-experiments/tracks/actors/EXP-0135.md).
- **R7 — existing workload/materiality contract:** [EXP-0136](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/.agents/skills/architecture-experiments/tracks/actors/EXP-0136.md).
- **R8 — physical-choice baseline and S/P amendment:** [EXP-0137](https://github.com/atmo-network/deos/blob/6d44e92d75e7334b5763a6684c62e5279c39725f/.agents/skills/architecture-experiments/tracks/actors/EXP-0137.md).
- **R9 — lifecycle close-allowance correction:** [commit 2d7b6f96](https://github.com/atmo-network/deos/commit/2d7b6f96e28d5a17dbf395ca4bcb7c459f1f37f0).
- **R10 — SDK background only:** official [`fungibles::Inspect`](https://paritytech.github.io/substrate/master/frame_support/traits/tokens/fungibles/trait.Inspect.html) distinguishes per-asset minimum, balance, total balance and reducible balance; official [`with_transaction`](https://paritytech.github.io/polkadot-sdk/master/frame_support/storage/transactional/fn.with_transaction.html) documents rollback and parent-transaction commit. These general APIs motivate the distinctions above; verify the actual pinned SDK and host adapter rather than importing a different version's behavior.
- **R11 — historical outstanding programmes:** [published v0.7.26 backlog](https://github.com/atmo-network/deos/blob/v0.7.26/BACKLOG.md), including its retained-programme references to earlier task contracts.
- **Planning provenance:** the previously supplied `DEOS_BACKLOG_0.7.27_LIVE_RING_FINAL.md` supplied the N-task identities. The working branch's already selected `Fixed`/`Percent` contract, completed narrow claims and current blockers take precedence over stale statements in that planning file.
