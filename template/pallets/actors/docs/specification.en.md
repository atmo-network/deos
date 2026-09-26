# DEOS Actors Specification

- **Scope**: Bounded economic actor runtime contract
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase.

---

## 1. Ownership and conventions

This specification owns Actor semantics, the public ABI shapes listed in it, and the resource guarantees every conforming implementation MUST provide. It does not own:

| Surface | Owner |
| --- | --- |
| Exact SCALE encoding, discriminants, and type paths | Runtime metadata |
| Storage items, keys, internal records, and page geometry | Generated storage descriptors and the package architecture document |
| Measured `Weight(RefTime, ProofSize)` values | Generated Weight descriptors |
| Concrete host adapter trait signatures | The package embedding guide |
| A host's concrete block allocation, fixed envelope, and reference constants | That host's integration documentation |

Each semantic function has exactly one owning section. Other sections use it only by reference, for example `Opening (§6.2)`. If two passages conflict, the owning section wins.

Rust blocks declaring public types, calls, events, or errors are normative shapes. Other code blocks are semantic pseudocode: they fix branch order, dependencies, and failure propagation, not identifiers or Rust syntax.

---

## 2. Core contract

### 2.1 Invariants

1. Equal canonical state and block context MUST produce equal behavior.
2. Every path MUST be `O(1)` or `O(K)` under explicit finite bounds.
3. The complete `Weight(RefTime, ProofSize)` of a transition MUST be admitted before semantic mutation.
4. An Actor Contract is a bounded linear sequence of `0..=MaxContractSteps` Steps (§4.1).
5. Contracts contain no loops, jumps, nested contracts, opaque dispatch, Task-authored memory, or authored whole-pipeline rollback.
6. The service quantum is Q1: one Actor MAY commit at most one Step in one block (§7.1).
7. A Pipeline MAY span multiple blocks and is non-atomic across committed Steps (§7.1).
8. An admitted Pipeline MUST NOT undergo economic apoptosis before its Cycle boundary (§10.4).
9. A cause observed in block `N` cannot authorize Actor execution before block `N + 1` (§3.2).
10. User and System Actors share one Service ring and class-neutral encounter order (§3.2).
11. Trigger occurrence, Pipeline Opening, and Action execution are independent transitions (§5.2, §6.2, §7.1).
12. Trigger, Pipeline Machine, and Action work have disjoint economic owners (§8).
13. Close deletes process semantics and MUST preserve sovereign custody (§10.3, §11.3).
14. Observations never activate Actors; they are read only by Step predicates at the actual check or Attempt (§5.1, §7.2).
15. Unknown capability, stale authority, unbounded work, invalid actual Weight, or unknown downstream failure MUST fail closed.
16. Rejected control transitions restore Actors state, events, scheduler state, and Actors fee movement to pre-state. Host nonce and ordinary transaction-payment effects are outside this definition.

### 2.2 Atomicity terms

| Term | Meaning |
| --- | --- |
| Rejected control transition | Call returns `Err`; Actors-owned state and events equal pre-state. |
| Provisional Task commit | Task layer succeeded inside the current Step transaction but is not durable until that transaction commits. |
| Committed unsuccessful attempt | The current Step transaction durably commits suspension or failure; fees owned by that committed transition remain charged. |
| Rolled-back scheduler attempt | The current Step transaction fails; residence, run, Task effects, Actors fees, and Actors events equal pre-attempt state. |

---

## 3. Current-state service

This section owns residence, service order, block rounds, wake acknowledgment, parking, and generation safety. Removed historical amount forms, Trigger histories, Opening snapshots, deferred Cycles, queue tickets, and per-Step successor tickets are invalid encodings, not compatibility aliases.

### 3.1 Residence states

Each Active Actor owns exactly one generation-bound process. A serving process has exactly one residence:

| Residence | Canonical obligation | Permitted exit |
| --- | --- | --- |
| `Live` | One Service ring membership for an admitted Pipeline or an Idle Actor due for a current start check. | One metered turn may retain Live, move to Deadline or Parked, disable, or close. |
| `Pending` | One Service ring membership for paid readiness or an owed current activation check; no Cycle admitted yet. | False refreshes parking evidence; true opens a Cycle no earlier than the next round; resource refusal retains Pending. |
| `Deadline` | One known future eligibility: a suspended retry or a timed review. | Due extraction returns the process to Service no earlier than the next round. |
| `Parked` | An Idle process with a generation-bound negative certificate for a parked-balance plan (§3.4). | A qualifying invalidation creates one coalesced `Pending` check. |
| `Disabled` | Serving authority is paused; no ordinary residence. | Only authorized control restores service; ordinary invalidation has no effect. |

A temporal Trigger (`AtTime`, `Cadenced`) MAY additionally own one independent Trigger deadline. That deadline is detector authority, not a residence, and cannot execute the Actor directly.

An open Cycle is Live when eligible by the next permitted round and in Deadline when eligibility is known later; it is never Parked. Unknown, stale, unavailable, or corrupt state cannot certify indefinite parking: it selects a typed deadline, deterministic timed review, or failure.

Every residence transfer is transactional: the complete transition commits, or the former residence remains unchanged. Exactly one source or destination obligation survives any failure. Every membership, wake, pending record, and deadline binds the exact generation; stale-generation work has no authority over a recreated Actor.

### 3.2 Block rounds and Service order

One block owns one immutable round identity shared by every Actor pass. The outcome MUST equal that of a snapshot of the ordered eligible membership taken at the round boundary, but implementations MUST NOT enumerate the ring to achieve it. A member receives at most one ordinary turn per round, and at most one Step commits for an Actor per block. A member admitted or reentered in block `B` is ineligible until `B + 1`. Removal never donates its turn; remove/reinsert and generation replacement cannot reset the same-block guards.

Admission fixes initial ring order; service then preserves cyclic encounter order among residents. A successful Step whose continuation is due next round retains membership and advances the Service cursor. Every newly admitted, reentered, or replacement membership is placed behind all current residents as encountered from the persistent next-encounter cursor. After `A` advances in `A -> B -> C`, admitting `D` before the next block yields next-round order `B -> C -> A -> D`.

Head, tail, and interior removal, Deadline transfer, parking, close, and additions preserve the encounter order of survivors. The valid Service head is authoritative: it cannot be bypassed, no cheap/heavy/System/User reordering exists, and when its complete transition does not fit, the pass stops and later residents remain untouched. Certified absence of current work is a distinct fully admitted transition and may detach the head. Empty, stale, already-served, and ineligible physical entries have explicit bounded traversal and cannot cause an infinite wrap.

Level-sensitive recurrence checks current state after Cycle completion no earlier than the next round and cannot start twice in one block. While an Actor is Running, in retry Deadline, Pending, or Live, repeated Manual or source hints coalesce and create no second Cycle, cursor reset, retry reset, or fee claim. Cadence misses coalesce into one current check without catch-up. One-shot temporal service has no recurrence after its completed or terminal Cycle.

### 3.3 Wake and acknowledgment

Invalidation records only that a recheck is owed; repeated updates coalesce. Registration, evaluation, and acknowledgment MUST be atomic or revisioned so an update before, during, or after evaluation cannot disappear: acknowledgment clears only the exact covered revision, and a later revision leaves the check owed. Pending saturation preserves one durable obligation. Disabled Actors and closed generations ignore ordinary wake hints.

Each transition pre-admits its complete multidimensional Weight and economic charge before mutation. Notification, current activation checks, residence transfer, Attempt and effect, resource refusal, and close are independently priced owners. No fee reserves future capacity or buys a second Cycle.

### 3.4 Parked-balance activation

`ParkedBalance` is an authored recurring activation mode for an Idle Parked generation. Its bounded plan names exact sovereign assets and a nonzero authored minimum delta for each. It excludes `Cadenced`; periodic recurrence remains a separate authored mode. A running Cycle, retry, Live continuation, Pending check, Disabled Actor, or closed generation owns no parked-balance registration.

The watched quantity is the host ledger's authoritative total owned balance of the exact sovereign account and asset. It includes held or frozen ownership and is distinct from spendable `Available`, which remains the only basis for Task amounts. A hold or freeze change alone does not qualify. A host that cannot expose this quantity and its minimum balance coherently MUST reject the mode.

For each watched asset `a`, with checked or widened arithmetic:

```text
floor[a]     = 100 * minimum_balance[a]
threshold[a] = max(authored_min_delta[a], floor[a])
delta[a]     = abs_diff(total_balance_now[a], anchor[a])
qualified    = any(delta[a] >= threshold[a])
```

Movement in either direction qualifies, equality at the threshold qualifies, and quantities of different assets are never summed. A missing, zero, or unrepresentable minimum rejects admission; a later minimum or plan change requires authorized reconfiguration under a new configuration identity. There is no saturation, unit substitution, or price conversion.

Activation and reconfiguration perform one atomic arm without execution: after all mutations of that transition commit, the current total balances become the anchor and the Park registration is published. Earlier deposits are never replayed as activation. After each completed balance-recurring Cycle, all effects, fees, and reservation releases commit before one atomic anchor capture and Park publication, starting a new parking episode; no intermediate sample, notice, or failed check moves the anchor.

A qualifying change creates one generation- and episode-bound Pending check; repeated notices coalesce. A negative check returns to Park with the same anchor and acknowledges only its covered revision; a later revision creates one new check, and a revision racing evaluation remains owed. A qualifying change that later reverses remains an owed check, but current conditions and current `Available` are revalidated before Cycle admission and effects. Adapters that publish per-asset causal revisions after committed transfer, mint, and burn provide event-driven invalidation; every other mutation route is covered by mandatory timed review.

### 3.5 Classification

One pure classifier owns Active-state classification. It validates canonical partitions (§4.5), `CycleState` against the run (§6.1), and current Contract, run, and admission bindings (§4.4), computes stored-state terminal predicates (§10.2), and selects the execution phase in this order: circuit breaker, pause, terminal, block/retry/cadence-tick wait, signal wait, ready. The phases are `GlobalCircuitBreaker`, `Paused`, `WaitingRetry`, `WaitingBlock`, `WaitingCadenceTick`, `WaitingSignal`, and `Ready`.

Opening-specific fee and nonce checks belong to Opening (§6.2), not the classifier. Classification errors are typed and MUST NOT become absence, waiting, or `Ready` (§13.4).

---

## 4. Actor Contract and canonical state

### 4.1 Public Contract model

```rust
struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy, ParkedBalance> {
  trigger: Trigger,
  cooldown_blocks: u32,
  window: Option<ScheduleWindow<BlockNumber>>,
  steps: Steps,
  funding: FundingPolicy,
  completion: CompletionPolicy,
  parked_balance_activation: Option<ParkedBalance>,
  auto_close_at_cycle_nonce: Option<u64>,
}

struct Step<Precondition, Task> {
  precondition: Option<Precondition>,
  task: Task,
  on_error: StepErrorPolicy,
}

struct ScheduleWindow<BlockNumber> {
  start: BlockNumber,
  end: BlockNumber,
}

enum CompletionPolicy {
  Persistent,
  CloseAfterProductiveCycle,
}

enum InitialLifecycle { Dormant, Active }
```

`steps.len()` MUST be in `0..=MaxContractSteps`; zero Steps is first-class (§6.5). A present `parked_balance_activation` MUST hold a nonempty bounded list of strictly asset-sorted unique watches with nonzero authored minimums, a nonempty Step plan, `Persistent` completion, and no cycle-nonce auto-close target; creation, activation, replacement, and decoding fail closed on any incompatible or noncanonical value.

Contract replacement replaces the complete authored value. Canonical equality is an exact no-op before rate limiting, cancellation, clocks, topology, writes, fees, or events.

### 4.2 Semantic identity

The semantic Contract ID is the protocol-fixed digest of the admitted canonical authored bytes only:

```text
SemanticContractId = Blake2_256(
  SCALE((
    b"DEOS_ACTOR_CONTRACT", // fixed [u8; 19]
    (
      trigger,
      cooldown_blocks,
      window,
      funding,
      completion,
      parked_balance_activation,
      auto_close_at_cycle_nonce,
    ),
    ordered_steps,
  ))
)

BodyCommitment = Blake2_256(
  SCALE((
    b"DEOS_ACTOR_BODY", // fixed [u8; 15]
    [(0u32, Step[0]), (1u32, Step[1]), ...],
  ))
)
```

Indexes MUST be contiguous and equal the exact Step count. Predicate canonicalization (§7.2) precedes identity derivation; whitelists are accepted only when already canonical (§5.1). Formatting, labels, storage wrappers, runtime state, topology, runtime Weight, and physical fragments MUST NOT affect either identity.

### 4.3 Contract storage guarantees

An admitted Contract is stored as a hot head plus a lazy tail. The head holds the authored header, Step count, Step 0 when present, both identities, admission authority (§4.4), and the per-Step resource envelopes. The tail holds Steps `1..N` in bounded, gap-free, non-overlapping chunks keyed by their first Step index; zero- and one-Step Contracts have no tail.

1. Ordinary execution loads only the head and, for Steps beyond 0, the one chunk holding the current Step.
2. Unreached Steps MUST NOT add execution ProofSize merely because they exist, and `MaxContractSteps` MUST NOT enlarge the maximum current-Step read.
3. Full reconstruction happens only in lifecycle mutation, runtime projection, integrity checks, and upgrades.
4. Every fragment binds the Actor, semantic Contract ID, body commitment, and exact index range; missing, stale, overlapping, foreign, or orphan fragments fail closed.

### 4.4 Admission authority and Pipeline envelope

Each admitted Contract carries a runtime-owned admission certificate binding at least the semantic Contract ID, body commitment, runtime semantics version, production Weight identity, storage geometry version, and configured bounds. It is derived authority, not authored state. A stale certificate fails closed until bounded re-certification or authorized Contract replacement; re-certification preserves semantic identity when authored bytes are unchanged.

Each Step carries one runtime-owned resource envelope with a maximum Actor Control Weight and a maximum Task-effect Weight, bound to its index, fragment, predicate and read geometry, Weight identity, and configured bounds.

The Pipeline envelope is the generated maximum **reachable** Actor Control work of one complete admitted Pipeline. It includes Opening exactly once (§6.2), each reachable current-Step control path (§7.1), retry control up to authored bounds (§7.5), and exactly one reachable completion or close branch including its cleanup (§6.6, §10.3). It excludes Trigger work (§5.2), Task effects (§12.1), and pre-Opening minimal apoptosis (§10.4). It MUST NOT be the sum of mutually exclusive Step maxima; Opening-only work is not multiplied by Step 0 retries and finalization-only work is not multiplied across Steps.

Create and semantic update are the only envelope producers. Opening reads the fixed-size envelope in `O(1)` and MUST NOT load tail chunks to quote Pipeline service (§6.2, §8.4).

### 4.5 Canonical facts and partitions

```rust
type ActorId = u64;
type OwnerSlot = u8;
type SystemSovereignId = u64;

enum ActorType { User, System }

enum ActorClass {
  User { owner_slot: OwnerSlot },
  System { sovereign_id: SystemSovereignId },
}

enum Mutability { Mutable, Immutable }
enum ActiveLifecycle { Active, Paused }
enum CycleState { Idle, Running, Suspended }

struct OutcomeTotals {
  executed_steps: u32,
  committed_effectful_tasks: u32,
  precondition_skips: u32,
  skipped_resolution: u32,
  skipped_funding_unavailable: u32,
  failed_steps: u32,
}

enum SuspensionReason { FundingUnavailable, Temporary }
enum CycleResult { Completed, Failed, Cancelled }
```

Each fact has one canonical owner; internal record layouts are implementation-owned:

| Fact | Canonical owner |
| --- | --- |
| Owner, class, mutability, sovereign account, latest terminated Cycle nonce, last control-mutation block | Actor identity |
| Authored Contract, identities, admission authority, envelopes | Contract head and tail (§4.3, §4.4) |
| Lifecycle, cycle phase, Trigger runtime state, latch, failure streak, schedule anchor, last Cycle block, terminal marker | Actor hot state |
| Open-Cycle cursor, outcomes, retry state, and Contract binding | Run state (§6.1) |
| Generation and residence | Actor process (§3.1) |
| Service ring, deadlines, park registrations, and reverse handles | Bounded scheduler topology |

`ActorType` is derived from `ActorClass` and MUST NOT be stored. Composite views and runtime API projections are read-only. No persistent cache may duplicate authored fields, Steps, cursor, outcomes, lifecycle, or latch.

Dormant means only the identity and its slot or locator exist. Dormant Actors own no Contract, hot state, process, run, detector membership, residence, Trigger deadline, or Active-state hold. Public creation admits Dormant only as Mutable; a host genesis configuration MAY declare a sealed Immutable System identity that can never activate or close through Actor control.

---

## 5. Trigger machine

### 5.1 Trigger families and runtime state

```rust
enum Trigger<AccountId, AssetId> {
  Manual,
  AddressEvent {
    source_filter: SourceFilter<AccountId>,
    asset_filter: AssetFilter<AssetId>,
  },
  AtTime { after_ticks: u64 },
  Cadenced { every_ticks: u64 },
}

enum SourceFilter<AccountId> {
  Any,
  OwnerOnly,
  Whitelist(BoundedVec<AccountId, MaxWhitelistSize>),
}

enum AssetFilter<AssetId> {
  Any,
  Whitelist(BoundedVec<AssetId, MaxWhitelistSize>),
}

enum FundingProvenance { Signed, InternalProtocol, Xcm }

struct AddressEvent<AccountId, AssetId, Balance> {
  destination: AccountId,
  source: Option<AccountId>,
  asset: AssetId,
  amount: Balance,
  provenance: Option<FundingProvenance>,
}

enum TriggerRuntimeState {
  Stateless,
  AtTime { anchor_tick: Option<u64>, consumed: bool },
  Cadenced { anchor_tick: Option<u64> },
}
```

`Manual` and `AddressEvent` require `Stateless`, `AtTime` requires AtTime state, and `Cadenced` requires cadence state; a mismatch is an Actor invariant failure (§13.4). Whitelists MUST be nonempty, duplicate-free, and strictly ordered by canonical typed SCALE bytes; admission rejects rather than normalizes noncanonical input.

Observation publication is never a Trigger source. An observation-reactive strategy composes a `Cadenced` or `AddressEvent` Trigger with a fresh-only observation precondition (§7.2).

### 5.2 Useful Trigger occurrence

A Trigger occurrence is exactly one family-specific cause that:

1. Passes source and detector authority;
2. Is semantically relevant;
3. Observes `pending_signal == false`;
4. Successfully charges the User Trigger fee, or is System fee-exempt (§8.3);
5. Commits `pending_signal: false -> true`;
6. Disables further Actor-specific occurrence work until re-arm (§5.3);
7. Emits `TriggerOccurrenceProcessed` (§13.2).

An occurrence is not a Cycle and does not charge Pipeline service (§6.2, §8.4). Source publication, movement, or explicit-call work has its own owner (§8.6, §12.4); the occurrence owns only Actor-specific detection, matching, and materialization attributable to `false -> true`.

### 5.3 Latch and redundant activity

`pending_signal` is the sole readiness latch:

```text
false -- useful occurrence --> true -- Opening --> false
```

While `pending_signal == true`, source activity MUST NOT create another occurrence, charge another Trigger fee, queue another Pipeline, accumulate Trigger history, or change any run cursor, current-state basis, or sovereign balance; where practical the Actor is absent from or disabled in the relevant detector topology. Source-owned state MAY keep changing, and economic ingress MAY still update custody (§7.4, §12.4).

The latch bounds one Actor to at most one open Pipeline and one paid Idle activation; no deferred Pipeline request or source history exists. No call clears the latch directly: Opening consumes it and deactivation or close deletes it (§6.2, §10.3). Running or Suspended source activity creates no latch and never alters the cursor, retry, or eligibility.

### 5.4 Trigger underfunding and source advancement

User Trigger underfunding means the sovereign account cannot pay the current Trigger fee (§8.3). It MUST NOT create free polling or repeated charging of the same cause.

| Family | Underfunded result |
| --- | --- |
| `Manual` | Reject the call; no Actors state, event, or Trigger fee change. Ordinary signed transaction payment remains. |
| `AddressEvent` | The certified movement and independent funding semantics MAY commit; no latch or Trigger fee. The movement is not retried as a Trigger. |
| `AtTime` | The one-shot source is consumed. A User Actor undergoes minimal apoptosis with `TriggerAdmissionInsufficient` (§10.4). System is fee-exempt. |
| `Cadenced` | The due point is skipped and the next future cadence point is installed; no latch or Trigger fee. |

Fee-collector failure is not underfunding. The Trigger charge reports `TriggerFeeCollectionFailed`, never `InsufficientFee`, when the collector rejects collection after capacity admission; the owning transaction preserves the exact source obligation and performs no occurrence mutation.

### 5.5 Trigger re-arm

Opening consumes the latch and re-arms the Trigger (§6.2):

| Family | Re-arm rule |
| --- | --- |
| `Manual` | Stateless and enabled. |
| `AddressEvent` | Stateless and enabled. |
| `AtTime` | Never re-arms; `consumed == true`. |
| `Cadenced` | Installs the first canonical cadence deadline strictly after the current authoritative tick; no catch-up. |

These rules also apply when Contract replacement preserves a latch acquired by the previous Trigger. A Running or Suspended Actor cannot acquire a latch for a second Cycle; after completion or abort, detector authority is derived again from current canonical state.

### 5.6 Causal cohorts

A causal cohort is a bounded ordered set of Actors affected by one shared Trigger cause, such as one Cadenced due tick. Cohort membership depends only on shared causal authority, never on Contract length, Actor class, Task kind, fee, or Weight.

An implementation MAY amortize source traversal, candidate classification, and occurrence materialization across a cohort when every member follows the same generated control branch. Cohorting MUST NOT reorder candidates, create Task-shape affinity, or merge per-Actor obligations: each Actor keeps its own Trigger fee, latch, residence, events, Pipeline fee, and Step outcome, and after Step 0 each nonterminal member follows independent Service scheduling. Unreached Pipeline length MUST NOT add first-reaction work (§4.3).

### 5.7 Family semantics and time bounds

- `Manual`: only the authorized owner may call `manual_trigger` (§13.1). A useful call is an occurrence (§5.2); a latched call is an exact Actor no-op that retains ordinary transaction payment (§8.3).
- `AddressEvent`: only certified positive non-self movement forms a cause (§12.4). Matching is independent of funding acceptance (§7.4). `SourceFilter::Any` accepts any source including absent; `OwnerOnly` requires a concrete source equal to the owner; `Whitelist` requires a concrete listed source. `AssetFilter::Whitelist` requires the exact listed asset.
- `AtTime`: `after_ticks > 0`. One relative consensus-time deadline that fires at most once, never catches up, and never re-arms.
- `Cadenced`: `every_ticks > 0`. Anchored at Active installation with ceiling quantization. A useful occurrence disables cadence detection until Opening re-arms it; missed latched-period points are forgotten.

`AtTime` and `Cadenced` use the host-configured consensus tick, disallow `cooldown_blocks` and `ScheduleWindow`, and MUST fit `MaxTemporalDelayTicks` (§14.3). Temporal arithmetic is constant-time and never iterates or replays missed points:

```text
first_due = ceil(now_millis / tick_millis) + period
next_due  = first canonical cadence point strictly greater than now_tick
```

At genesis, temporal anchors are uninitialized; the first temporal service after the timestamp inherent installs the full future deadline without creating readiness.

For signal-driven Triggers, block eligibility is:

```text
cooldown_anchor = last_cycle_block.or(schedule_anchor)

cooldown_eligible_at =
  schedule_anchor                                      for the first Cycle
  checked_add(cooldown_anchor, cooldown_blocks)        otherwise

signal_eligible_at(cause_floor) =
  max(cause_floor, cooldown_eligible_at, window.start when present)
```

A `ScheduleWindow` is inclusive. Validation requires `end > start`, representable `end + 1`, inclusive length `>= MinWindowLength`, current block `<= end`, bounded future start delay, and first possible eligibility `<= end`. `terminal_at = end + 1` (§10.2). Active installation sets `schedule_anchor = max(now, window.start)` for a future window, otherwise `now`.

### 5.8 Detection geometry

Detection is source-specific: `Manual` through the direct call, `AddressEvent` through the certified destination and filter path, and `AtTime`/`Cadenced` through the temporal deadline index. A universal Trigger index is forbidden.

Detector workers MUST preserve source revision and time order, inspect only exact affected candidates, admit complete multidimensional Weight before mutation, use bounded candidate and page counts, preserve one exact source obligation on refusal, and never skip corruption or let later source work overtake it.

---

## 6. Pipeline and Cycle

### 6.1 Cycle and run state

A run exists iff `CycleState` is `Running` or `Suspended`. `Running` has no suspension, `Suspended` has exactly one `SuspensionReason`, and `Idle` has no run. The run binds the exact Contract, admission identity, and paid Pipeline envelope, and holds the open Cycle nonce, the cursor (the sole current Step index), unsuccessful attempts at the cursor, last attempt and committed-Step blocks, the eligibility block, cumulative outcomes, and the last Step outcome. Completed-run history is not retained in consensus state.

### 6.2 Opening

Opening is the only transition that consumes paid readiness into one Cycle. It MUST execute in this order:

1. Validate the Actor, latch, residence, Contract, admission, and Pipeline envelope (§3.1, §3.5, §4.4).
2. Apply Opening terminal checks (§10.2).
3. Validate that the active-installed maximum run-state hold is present (§8.2).
4. Check and charge the complete Pipeline Machine fee (§8.4).
5. Derive `run.cycle_nonce = identity.cycle_nonce + 1` without changing the identity nonce (§6.4).
6. Consume `pending_signal` (§5.3) and re-arm the Trigger (§5.5).
7. Emit `CycleStarted` (§13.2).
8. For a nonempty Contract, execute Step 0 in the same current-Step transaction (§7.1); for a zero-Step Contract, finalize atomically (§6.5).

Opening MUST NOT partially occur. Insufficient Pipeline payment invokes minimal apoptosis with `CycleAdmissionInsufficient` (§10.4); inconsistent hold authority fails closed as an Actor invariant; the prior Trigger fee remains final (§8.3). A Pipeline fee-collection failure despite valid capacity rolls back the entire Opening, keeps the latch consumable, and preserves the Service head.

### 6.3 Current Attempt inputs

Cycle admission captures no predicate result, amount basis, or funding history. Each Attempt loads only its current Step and reads every predicate and dynamic amount from current authoritative state after its fee reservation; a retry repeats those reads. Every amount is exactly `Fixed(value)` or `Percent(perbill)`: `Fixed` is checked against current capacity without clipping, and `Percent` uses widened floor arithmetic over current `Available`. Every debit preserves the current Action-fee reservation and protected minimum (§8.5, §11.4).

### 6.4 Cycle nonce

The identity nonce is the latest terminated Cycle nonce. Opening derives `run.cycle_nonce = checked_add(identity.cycle_nonce, 1)` and does not mutate the identity nonce. Completion, failure, cancellation, or close assigns `identity.cycle_nonce = run.cycle_nonce` exactly once. Retry, suspension, Step progress, breaker deferral, and congestion change neither nonce.

### 6.5 Zero-Step Cycle

A zero-Step Contract has no Task, Step event, tail, or run. A ready zero-Step Cycle consumes one Service encounter and atomically performs Opening (§6.2), charges the zero-Step Pipeline Machine fee (§8.4), emits `CycleStarted`, commits the new nonce (§6.4), emits `CycleSummary(Completed)`, and applies terminal policy (§10.2). `auto_close_at_cycle_nonce = Some(1)` makes a fresh zero-Step Contract one-shot.

### 6.6 Completion and cancellation

A Cycle completes at plan end or on successful `StopCycle` (§7.6). Finalization MUST record the final Step outcome when present (§7.5), emit the boundary events (§13.2), commit the run nonce (§6.4), delete the run while preserving the active-installed run-state hold (§8.2), update `last_cycle_block`, apply terminal precedence (§10.2), and otherwise return to Idle and recompute the Idle residence. Source activity that occurred while Running or Suspended creates no second Cycle or deferred readiness.

Cancellation deletes the run without compensating earlier committed Steps: it emits `CycleCancelled`, then `CycleSummary(Cancelled)`, commits the run nonce, and deletes the run. Contract replacement, deactivation, explicit cancellation, close, and incompatible upgrade own their cancellation reasons (§10.3, §10.5, §14.2).

---

## 7. Step execution

### 7.1 One-Step attempt transaction

An Attempt is one admitted execution of exactly one current Step; a retry is a later Attempt at the same cursor. Before reading any predicate or dynamic amount, the scheduler MUST reserve the current Step's maximum Actor Control Weight and maximum Task-effect Weight (§9.1) and, for User Actors, the maximum current Action fee (§8.5). If any reservation does not fit, the Attempt defers without evaluation or mutation.

One current-Step transaction atomically owns:

```text
generation/process/Service/Contract validation
+ optional Opening for cursor 0
+ current-Step precondition
+ amount resolution
+ at most one Task effect
+ Action fee settlement when a Task is invoked
+ Step outcome and counters
+ cursor, suspension, completion, or close transition
+ retained Service, Deadline, Parked, Disabled, or close placement
+ Actors events
```

A Task MUST NOT be split across blocks, and earlier committed Steps are never rolled back by later failure. After a committed Step, `last_committed_step_block` equals the current block; a second Step commit for the same Actor in that block MUST fail closed independently of residence correctness.

An advancing Step with a successor sets eligibility to `current_block + 1` and retains one Live Service obligation (§3.1); it does not load or execute the successor. A retry suspension atomically moves the same process from Service to its exact Deadline.

### 7.2 Precondition

```rust
struct Precondition<P, MaxClauses, MaxPerClause> {
  clauses: BoundedVec<BoundedVec<P, MaxPerClause>, MaxClauses>,
}
```

```rust
enum PredicateError { InvalidObservation }

enum Predicate<AssetId, Balance, BlockNumber, FeedId> {
  BalanceAbove { asset: AssetId, threshold: Balance },
  BalanceBelow { asset: AssetId, threshold: Balance },
  BalanceEquals { asset: AssetId, threshold: Balance },
  BalanceNotEquals { asset: AssetId, threshold: Balance },
  BlockNumberAbove { threshold: BlockNumber },
  BlockNumberBelow { threshold: BlockNumber },
  ObservationAbove { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationBelow { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
  ObservationNotEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
}
```

A present Precondition is nonempty DNF: outer clauses are OR, inner predicates are AND. Admission canonicalizes predicates and clauses by canonical typed SCALE order, rejects duplicate clauses after predicate deduplication, and absorbs exact supersets under `A OR (A AND B) = A`.

Every admitted predicate is evaluated with no short-circuit, because Weight MUST be data-independent. Evaluation happens immediately before the owning Step and observes prior committed Steps plus intervening external state.

`Above` is strict `>` and `Below` is strict `<`. An `Unavailable`, `Uninitialized`, or `Stale` observation evaluates false; a structurally invalid `Fresh` observation is `PredicateError::InvalidObservation`, a Permanent Step failure. `max_age_blocks` MUST be positive. Balance predicates read only the Actor sovereign account, subtract the current Action-fee reservation for the fee-native asset, and grant no spending authority (§8.5, §11.4). A false Precondition emits `StepSkipped(PreconditionFalse)` and advances (§7.5).

### 7.3 Amount resolution

```rust
enum AmountResolution<Balance> {
  Fixed(Balance),
  Percent(Perbill),
}
```

Each Task amount resolves to `Resolved(positive value)`, `Skipped`, or `FundingUnavailable`:

- Percentages use widened floor division, and a dynamic zero is `Skipped`;
- A positive exact debit above current capacity is `FundingUnavailable` and MUST NOT be silently reduced;
- `Percent(100%)` is the only whole-current-available form and has no lifecycle privilege.

For multiple amount fields: any `FundingUnavailable` gives `FundingUnavailable`, else any `Skipped` gives `Skipped`, else the Task is executable with all values.

| Resolution surface | Tasks | Current basis |
| --- | --- | --- |
| Preserve-source | Transfer, SplitTransfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | current preservable balance |
| Output-target | Mint, SwapOut | authored output; percentage amounts forbidden |
| Share-spend | Unstake | current staking shares |

### 7.4 Funding authorization

```rust
enum FundingSourcePolicy<AccountId> {
  OwnerOnly,
  SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>),
  RuntimePolicy,
  AnyVerifiedIngress,
}
```

A positive certified credit is authorized only when the funding policy accepts its source and provenance:

| Policy | Accepted source |
| --- | --- |
| `OwnerOnly` | signed provenance and concrete source equal to owner |
| `SignedAllowlist` | signed provenance and concrete allowlisted source |
| `RuntimePolicy` | the configured authority accepts the exact source/provenance pair; all-`None` is denied |
| `AnyVerifiedIngress` | a concrete source or typed provenance exists; all-`None` is denied |

Rejected credit remains custody only; Trigger matching is independent (§5.7). Actors retain no amount accumulator or funding history: every later amount resolution reads current `Available`. Close and deactivation move no custody (§10.3).

### 7.5 Step outcome and error policy

```rust
enum StepErrorPolicy {
  AbortCycle,
  ContinueNextStep,
  RetryLater { max_attempts: u32 },
}

enum StepSkippedReason { PreconditionFalse, ResolutionSkipped, FundingUnavailable }

enum StepOutcome {
  Executed,
  Stopped,
  Skipped(StepSkippedReason),
  FundingUnavailable,
  Failed(TaskFailure),
}
```

`RetryLater.max_attempts` counts the first unsuccessful execution and all retries and MUST be in `2..=MaxRetryAttempts`.

Each step whose enclosing scheduler attempt commits selects exactly one row from the closed transition table:

| ID | Step result | Policy | Durable transition |
| --- | --- | --- | --- |
| `ST-01` | Precondition false | any | Record skip; advance or complete. |
| `ST-02` | Resolution `Skipped` | any | Record skip; advance or complete. |
| `ST-03` | `FundingUnavailable` | `AbortCycle` or `ContinueNextStep` | Record funding skip; advance or complete. |
| `ST-04` | `FundingUnavailable` | `RetryLater`, below bounds | Suspend same cursor. |
| `ST-05` | `FundingUnavailable` | `RetryLater`, local or global bound reached | Record terminal failure. |
| `ST-06` | Effect success | any | Record effectful success; advance or complete. |
| `ST-07` | `StopCycle` success | any | Record stopped; complete immediately. |
| `ST-08` | Temporary failure | `ContinueNextStep` | Record failure; advance or complete. |
| `ST-09` | Temporary failure | `AbortCycle` | Record failure; terminate Failed. |
| `ST-10` | Temporary failure | `RetryLater`, below bounds | Record failure; suspend same cursor. |
| `ST-11` | Temporary failure | `RetryLater`, local or global bound reached | Record terminal failure. |
| `ST-12` | Permanent failure or predicate error | `ContinueNextStep` | Record failure; advance or complete. |
| `ST-13` | Permanent failure or predicate error | `AbortCycle` or `RetryLater` | Record failure; terminate Failed. |

A step increments at most one primary outcome counter. The deltas are exact: precondition skip increments `precondition_skips`; resolution skip increments `skipped_resolution`; advancing funding skip increments `skipped_funding_unavailable`; successful effect increments `executed_steps` and `committed_effectful_tasks`; successful `StopCycle` increments `executed_steps`; invoked Task failure increments `failed_steps`.

`FundingUnavailable` is not a Task failure, and `AbortCycle` aborts on Task failure only. A retry suspension increments the cursor-local and global unsuccessful counters exactly once; the local bound wins when both are reached by the same Attempt. A terminal Failed Attempt increments the global streak once; a `ContinueNextStep` failure, deferral, pause, cancellation, exact no-op, and advancing funding skip do not. A completed Cycle resets the global streak.

```text
next_local  = previous suspension at same cursor ? previous + 1 : 1
next_global = unsuccessful_attempt_streak + 1
backoff(i)  = min(2^i, 8) blocks

retry_eligible_at =
  last_attempt_block + max(cooldown_blocks, backoff(next_local - 1))
  bounded by [window.start, terminal_at] when a ScheduleWindow is present
```

A retry suspension inside an active window therefore wakes at the window terminal rather than sleeping past it, and the persisted run eligibility equals the exact Deadline destination.

### 7.6 Task semantics

```rust
struct SplitLeg<AccountId> { to: AccountId, share: Perbill }

enum InputLimit<Balance> { LiveQuote, Absolute(Balance) }

enum Task<AccountId, AssetId, Balance> {
  Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
  SplitTransfer { asset: AssetId, amount: AmountResolution<Balance>, legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs> },
  SwapIn { asset_in: AssetId, amount_in: AmountResolution<Balance>, asset_out: AssetId, slippage_tolerance: Perbill },
  SwapOut { asset_out: AssetId, amount_out: AmountResolution<Balance>, asset_in: AssetId, input_limit: InputLimit<Balance>, slippage_tolerance: Perbill },
  AddLiquidity { asset_a: AssetId, asset_b: AssetId, amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance>, min_lp_out: Balance },
  RemoveLiquidity { lp_asset: AssetId, asset_a: AssetId, asset_b: AssetId, lp_amount: AmountResolution<Balance>, min_amount_a: Balance, min_amount_b: Balance },
  Burn { asset: AssetId, amount: AmountResolution<Balance> },
  Mint { asset: AssetId, amount: AmountResolution<Balance> },
  Stake { asset: AssetId, amount: AmountResolution<Balance> },
  DonateLiquidity { asset_a: AssetId, asset_b: AssetId, max_amount_a: AmountResolution<Balance>, max_ratio_error: Perbill },
  Unstake { asset: AssetId, shares: AmountResolution<Balance> },
  StopCycle,
}
```

- At most one Task effect is invoked per Attempt (§7.1), through the canonical host operation (§12.1).
- `Mint` is System-only. Self-transfer and duplicate split recipients are invalid.
- Each Task contains at most two `AmountResolution` fields, and every debit preserves the protected minimum (§11.4).
- `Transfer(Percent(100%))` has no close-specific privilege.
- `CloseAfterProductiveCycle` observes only committed effectful Tasks (§10.2).

`SplitTransfer` requires `2..=MaxSplitTransferLegs` legs, positive unique shares, and total share `<= 1`. Each leg is floored and rounding dust remains with the Actor. Certified ingress to each recipient belongs to this Task effect, including synchronous finalization of a recipient whose window has expired (§12.4).

`SwapIn` and `SwapOut` use current executable quotes inside the adapter boundary (§12.4); `InputLimit::Absolute` is a cap, not an admission gate. `AddLiquidity` amounts are debit caps and actual used amounts and LP output are returned; `RemoveLiquidity` debits the exact resolved LP amount. `DonateLiquidity` uses one authored asset-A cap plus one current derived cap on pre-existing asset-B debit; the adapter may acquire asset B within the atomic operation, so returned donated B may exceed that debit cap.

---

## 8. Economics

### 8.1 Economic surfaces

Actors has four service-fee boundaries and one refundable state resource:

```text
committed User creation        -> ActorCreationFee
useful Trigger occurrence      -> TriggerFee
Pipeline Opening               -> PipelineMachineFee
invoked Action attempt         -> ActionExecutionFee
retained User Actor state      -> ActorStateHold
```

Ordinary transaction payment remains owned by the host. Task-native protocol fees remain owned by their mechanism (§12.1). System Actors are exempt from Actors Creation, Trigger, Pipeline, and Action fees but consume identical block resources; Task-native fees MAY still apply. Every Actors fee transfer is ledger-only and MUST NOT create AddressEvent ingress, funding, readiness, or placement.

### 8.2 Creation fee and state hold

`ActorCreationFee` is a fixed nonrefundable process-admission and anti-spam charge, paid by the signed creator only when User creation commits. It is not a second payment for create-call Weight. It economically backs the protocol obligation to perform minimal pre-Opening apoptosis (§10.4) and prepays no Trigger, Pipeline, or Action service; minimal apoptosis remains an obligation even if later cleanup cost exceeds the historical fee.

`ActorStateHold` is a refundable hold on the User owner account backed by actual retained geometry; it is not a fee and not rent:

- Elapsed time does not change it and no recurring collection exists;
- Zero- and one-Step Contracts MUST NOT reserve a maximum-size body footprint;
- Active installation reserves one type-derived maximum run-state hold before autonomous service becomes possible, never a hand-maintained byte constant;
- Opening, Step progress, retry, suspension, Cycle boundary, and cancellation MUST NOT mutate or release that run-state hold;
- Deactivation releases Active Contract, topology, funding, and run holds atomically and retains only the Dormant identity hold; close releases every Actor-state hold (§10.3);
- Inability to reserve rejects creation, activation, or replacement atomically;
- Sovereign custody is not Actor state and is never held by this mechanism (§11.3).

System state is host-owned bounded capacity and MAY be hold-exempt; exemption grants no resource or scheduler preference.

### 8.3 Trigger fee

```text
TriggerFee = WeightToFee(maximum generated control Weight of one useful family-specific occurrence)
```

A User Trigger fee is charged exactly once, from fee-native balance above `MinUserBalance`, before `pending_signal: false -> true` (§5.2, §11.4). Redundant latched-period activity charges nothing (§5.3). A useful User `manual_trigger` charges the complete generated Manual occurrence owner to the sovereign account and returns `Pays::No`; a redundant, rejected, underfunded, or System Manual call retains ordinary transaction payment and charges no Trigger fee. Automatic source producers pay their own source work; the Actor pays only its occurrence. Committed Trigger fees are nonrefundable and independent of later Pipeline admission.

### 8.4 Pipeline Machine fee

```text
PipelineMachineFee = WeightToFee(Pipeline envelope, §4.4)
```

The User fee is charged once at Opening from fee-native balance above `MinUserBalance` (§6.2, §11.4). It prepays all reachable Actor Control work of that Pipeline through its Cycle boundary, including exactly one reachable completion or close branch with its cleanup. Quotes MAY report the machine and cleanup components separately, but there is no separately charged cleanup fee, per-Step machine charge, remaining machine budget, hold or refund settlement, Trigger work, or Task-effect work in it. Running and Suspended Attempts consume paid machine authority without a balance predicate.

Insufficient capacity causes minimal apoptosis before Opening (§10.4); collector failure rolls back Opening and preserves the latch (§6.2). Committed Pipeline fees are nonrefundable.

### 8.5 Action fee

Before an Action-bearing Task is invoked, the Attempt reserves the maximum Task-effect fee from the User Actor's fee-native balance above `MinUserBalance`:

```text
cannot reserve -> FundingUnavailable; no Action invocation; no Action fee
can reserve    -> invoke canonical effect
                  -> settle valid actual effect Weight
                  -> charge actual Action fee on success or typed failure
```

A retry is a new independently charged Action attempt. Before a Suspended User Actor receives retry execution, the scheduler proves that its fee-native balance above `MinUserBalance` covers the current Action maximum; otherwise it selects fee-free custody-neutral minimal apoptosis before invocation (§10.4). Effect and actual-fee settlement form one transaction, so unused liability is released and no debt survives rollback. False precondition, skipped resolution, `FundingUnavailable`, and `StopCycle` invoke no Action and charge no Action fee. Invalid or greater-than-reserved actual Weight rolls back the current-Step transaction and fails closed.

### 8.6 No double charging

No two economic surfaces may charge the same Weight or retained byte:

| Work | Sole economic owner |
| --- | --- |
| External call dispatch and origin work | ordinary transaction fee |
| User process admission | Actor Creation Fee (§8.2) |
| Retained Actor state | Actor State Hold (§8.2) |
| Useful readiness `false -> true` | Trigger Fee (§8.3) |
| Complete admitted Pipeline control | Pipeline Machine Fee (§8.4) |
| Invoked Task effect | Action Fee (§8.5) |
| Router trading, staking, and other native fees | underlying host mechanism (§12.1) |

---

## 9. Block resources and service passes

### 9.1 Meters

Block Weight after the host's fixed and context envelope is schedulable and splits component-wise into two non-convertible meters:

```text
ActorControlLimit   = floor(SchedulableBlockWeight * host_control_share)
SharedEconomicLimit = SchedulableBlockWeight - ActorControlLimit
ActorBaseTurn       = floor(SharedEconomicLimit / 2)
UserBaseTurn        = SharedEconomicLimit - ActorBaseTurn
```

The host configures `host_control_share` in `(0, 1]`; its integration documentation owns the selected value and the fixed envelope. Both components are compared, reserved, and released independently; neither converts into the other.

Actor Control pays only Actor-specific work: detection and occurrence materialization, latch and placement, Opening, precondition and amount evaluation, run persistence, retry, completion, close control, scheduler topology, and bounded cleanup. Shared Economic pays ordinary external economic dispatch and Actor Task effects; a Task effect and its external equivalent use the same host mechanism and effect Weight (§12.1). Actor Control MUST NOT borrow Shared Economic capacity and MUST NOT be lent to it; actual-Weight reclaim never moves Weight between the meters.

### 9.2 Prepass, base pass, external phase, and Drain

Every block contains exactly one mandatory Actor Prepass inherent after required context inherents and before ordinary external extrinsics, including a block with no Actor work. It carries no author-selected scheduling, budget, or payload data. The Prepass:

1. Services bounded due Deadline and detector obligations caused before the current block boundary (§5.8);
2. Materializes eligible readiness (§5.2);
3. Opens or resumes the block's Service round at the persistent ring cursor (§3.2);
4. Runs the Actor base pass: encounters residents in ring order and admits each complete transition before mutation, stopping at the first head whose control or effect envelope does not fit the remaining Actor Control meter or `ActorBaseTurn`.

Ordinary external extrinsics then consume Shared Economic capacity, including any `ActorBaseTurn` the base pass left unused. After external dispatch, Actor Drain MAY continue from the same cursor and round using the remaining Actor Control meter and remaining Shared Economic capacity. Drain MUST NOT execute current-block readiness, grant a second Step to one Actor, reorder the head, or exceed the actual remainder.

Current-block publications use next-block eligibility, and a resident already considered in the current round cannot execute again; together these prevent same-block Actor recursion without any per-Step ticket. A maximum valid Step may be the only Actor Step in a block: this is paid bounded service, not starvation. Pipeline- or Action-fee collector failure rolls back the current transition, preserves the Service head, and stops the pass; it is not a Task failure.

### 9.3 Liveness

Given a finite bounded Service population, recurring conforming Actor Control and Shared Economic capacity, finite stale churn, eventual placement capacity, successful fee collection when valid payment capacity exists, and no structural invariant fault, all eligible residents receive service in persistent ring order. No fixed block latency is promised; a larger runnable population MAY lengthen inter-Step gaps while preserving order and eventual service.

Without successful fee collection, the Service head MAY block later service despite spare Weight; only restored prerequisites or already-authorized lifecycle transitions resolve that obstruction. Starvation telemetry MUST NOT change priority, order, or execution authority.

---

## 10. Lifecycle

### 10.1 Class and mutability

User creation is signed; the caller becomes owner and consumes one owner slot (§11.2). System creation requires `SystemOrigin` and consumes a System locator (§11.2).

| Actor | Authorized control |
| --- | --- |
| User | signed owner |
| System | signed owner or `SystemOrigin` |

A User Mutable Actor MAY update its Contract, pause and resume, cancel a run, deactivate, reactivate, and explicitly close. Public Dormant creation requires `Mutable`. Public Immutable creation MUST install an Active Contract because no later activation authority exists. A host genesis configuration MAY declare an Immutable Dormant System identity as a permanently sealed role; activation and owner close MUST reject it.

A User Immutable Actor rejects Contract replacement, pause and resume, activation and deactivation, cancellation, and owner close; MAY author a Manual Trigger; cannot author `RetryLater`; MUST set `auto_close_at_cycle_nonce = Some(1)` when authoring `AtTime`; and terminates only through authored or lifecycle terminal rules, one-shot Trigger underfunding (§5.4), Pipeline-admission apoptosis (§10.4), or deployed-lineage upgrade (§14.2).

A System Immutable Actor rejects actor-scoped control, cannot author Manual or `RetryLater`, is Actors-fee-exempt, and remains Active until an independently owned terminal rule or upgrade removes it.

Immutability fixes policy and owner control, not runtime Weight, fee conversion, adapter behavior, or host economics. `auto_close_at_cycle_nonce` changes only through complete Mutable Contract replacement and MUST be strictly above the current terminated nonce within `MaxAutoCloseNonceHorizon`.

### 10.2 Terminal precedence

Terminal predicates are evaluated only by their owning transition:

| Transition | Precedence |
| --- | --- |
| Stored-state classifier (§3.5) | `WindowExpired`; `RetryAttemptsExhausted`; `ConsecutiveFailures`; `AutoCloseNonceReached` while Idle; already-materialized permanent `SchedulerIndexExhausted` |
| Opening (§6.2) | stored-state terminal reason; `CycleNonceExhausted`; `CycleAdmissionInsufficient` when the Pipeline Machine fee cannot be provided |
| Step finalization (§7.5) | retry-local bound; global failure bound; `ProductiveCycleCompleted`; `AutoCloseNonceReached` after a non-suspended Cycle boundary; permanent deadline-placement `SchedulerIndexExhausted` |
| Temporal Trigger service (§5.4) | `TriggerAdmissionInsufficient` for a due User `AtTime` source that cannot pay its Trigger fee |

`CloseAfterProductiveCycle` closes only after a complete Cycle reaches `Completed` with `committed_effectful_tasks > 0`. Active-state hold inconsistency at Opening fails as an Actor invariant rather than a close reason.

### 10.3 Close

Close is lifecycle cleanup only. It MUST atomically cancel any open run (§6.6), revoke execution authority, remove the Contract, admission authority, hot state, run, detector memberships, residence, deadlines, reverse handles, and holds, release the User slot or mark the System locator vacant (§11.2), and emit `ActorClosed` (§13.2).

Close MUST NOT enumerate assets, transfer balances, unwind positions, invoke a Task, select a refund recipient, or alter sovereign custody. Explicit User close is available only to the signed owner of a Mutable User Actor; its ordinary transaction fee is paid by the caller and requires no Actor solvency.

### 10.4 Minimal User apoptosis

Minimal User apoptosis is automatic process-state collection with no custody effect. It occurs only for `TriggerAdmissionInsufficient` from one-shot User `AtTime` (§5.4), `CycleAdmissionInsufficient` before Opening (§6.2), or `CycleAdmissionInsufficient` before a Suspended User retry whose current maximum Action liability is unavailable (§8.5).

It MUST NOT occur after an Action effect or leave fee debt. Trigger and Pipeline funding never close a Running or Suspended Pipeline; only the bounded pre-invocation retry-liability rule may close a Suspended User Actor. Minimal apoptosis performs Close (§10.3) without a Task, custody scan, fee reserve, or economic policy; its cleanup obligation is backed by the committed Creation Fee (§8.2).

### 10.5 Control transitions and circuit breaker

Contract replacement, deactivation, explicit run cancellation, close, and incompatible upgrade MUST cancel the open run before changed meaning becomes executable (§6.6).

Pause preserves the Contract, any open run, latch, clocks, failure state, and Trigger detector evolution, and moves the process to `Disabled`. Resume reconstructs the exact required residence (§3.1). An Idle paused Actor may retain one already-paid latch, but source activity creates no deferred readiness and no Step executes.

Active creation or activation installs a new Active epoch with `Active/Idle`, no run, `pending_signal = false`, zero failure streak, current Trigger runtime state, schedule clocks, terminal marker, generation-bound residence, detector topology, and exact state hold. Activation preserves the Dormant identity nonce. Deactivation cancels any run and removes the Active epoch while preserving identity, slot or locator, nonce, custody, and the persistent control clock. Dormant activation with an exhausted Cycle nonce closes the identity with `CycleNonceExhausted` instead of installing an unusable epoch.

Authorized Mutable control MAY repair a non-window stored terminal condition before the scheduler or sweep commits Close; `WindowExpired` substitutes Close before the requested mutation. Mutable control is limited to one committed semantic mutation per Actor per block; an exact no-op returns before rate limiting.

While the global circuit breaker is active, Service Step effects and ordinary automatic terminal close do not run; mandatory minimal apoptosis, explicit close, bounded sweep, detector, Deadline, and stale-cleanup work MAY run; new Active creation and activation fail; authorized control over existing Mutable Actors remains available; and Service order and placement remain unchanged.

Permissionless sweep is bounded and closes only stored-state terminal reasons (§10.2); it never predicts Trigger or Pipeline affordability. Actor-targeting control and certified ingress may substitute Close only for `WindowExpired`.

---

## 11. Sovereign custody and identifiers

### 11.1 Sovereign-account derivation

```text
User seed   = Blake2_256(SCALE(ActorsPalletId, b"user", owner, owner_slot))
System seed = Blake2_256(SCALE(ActorsPalletId, b"system", system_sovereign_id))
account     = HostSovereignAccountDeriver(seed)
```

Derivation MUST be total, deterministic, class-separated, and stable for every previously admitted custody identity.

### 11.2 User slots and System locators

User slots are owner-local with `0 < MaxOwnerSlots <= 255`; `u8::MAX` is never a valid slot. Default User creation selects the lowest free slot, exact-slot creation requires that exact free slot, and User close releases it. System creation allocates a unique locator and System close marks it vacant; locator reuse creates a new Actor id with the same sovereign account. Actor ids never repeat, and every Active epoch carries a checked generation.

Sovereign custody at an unindexed derived account does not block exact reattachment. Reserved-account and live-collision errors remain distinct (§13.4). A previously registered vacant System locator remains reattachable even if later host policy classifies its account as reserved; the exception applies only to that locator. Custody identity survives host account-provider removal, although host dust or reaping policy may remove value independently.

### 11.3 Custody reattachment and recovery

Process lifetime and custody lifetime are independent. After Close the sovereign account, its balances, and adapter-exposed positions remain host-ledger state, and no Actors authority exists until reattachment. A fresh User Actor reattaches the same custody iff owner and exact slot equal those of the closed Actor; a fresh System Actor reattaches iff it reuses the same vacant locator. Reattachment inherits custody only, never Contract, mutability, nonce, Trigger state, lifecycle, run, or guarantees.

Recovery uses ordinary authored Contracts and Tasks (§7.6). Clients MAY automate exact-slot recreation, funding, recovery Contract generation, withdrawal, and re-close; Actors provides no direct recovery-transfer call.

### 11.4 Protected minimum

```text
protected_minimum   = MinUserBalance            for User Actors and the fee-native asset
protected_minimum   = host asset minimum balance otherwise
spendable_balance   = balance - current_action_fee_reservation
preservable_balance = spendable_balance - protected_minimum
```

Subtraction saturates only where shown. Every authored debit, including `Percent(100%)`, uses preservable capacity, and no lifecycle branch grants a source-exhaustion privilege.

---

## 12. Host effects and adapters

### 12.1 Canonical Task-effect ownership

Each effectful Task invokes the same canonical host economic mechanism as its external equivalent, with identical state-transition owner, atomicity boundary, synchronous consequences, typed failure classification, and production effect Weight. Actors MAY use narrow typed ports and need not construct a runtime call, but a cheaper shadow mechanism is forbidden. Actors owns orchestration control only; the host mechanism owns Task-effect Weight and Task-native fees.

### 12.2 Adapter obligations

The host provides asset, DEX, liquidity, staking, fee-collection, observation, ingress, and account-policy adapters; the package embedding guide owns their exact trait surface. Every adapter MUST be deterministic and bounded, return typed `TaskFailure { error, retry: Temporary | Permanent }` rather than panicking, report actual Weight no greater than its reserved maximum, and fail closed when a mutation capability is missing. Fee collection is one atomic ledger-only movement into the host's collection account.

### 12.3 Failure classification

Retryability MUST NOT derive from strings, module indexes, or broad token errors. Temporary covers dynamic slippage, current quote or cap insufficiency, stale reference, liquidity movement, recipient deposit unavailability, and recoverable placement capacity. Permanent covers malformed configuration, invalid provenance, missing static capability, monotonic namespace exhaustion, topology corruption, and invariant failure. Unknown downstream error is Permanent. `FundingUnavailable` is an amount or Action admission outcome, not a `TaskFailure` (§7.3, §8.5).

### 12.4 Special rules

**Swap.** The DEX adapter obtains a current executable quote internally, applies the authored cap and tolerance, invokes the canonical router, validates actual amounts, and returns typed failure; Actors exposes no independent quote surface. For `SwapOut`:

```text
capacity_cap     = preservable input balance
authored_cap     = capacity_cap                       for LiveQuote
                   min(capacity_cap, absolute_cap)    for Absolute
effective_max_in = min(authored_cap, quote + ceil(quote * tolerance))
```

A zero or insufficient cap is `FundingUnavailable` or Temporary according to whether dispatch was invoked. `SwapIn` requires positive actual output no worse than the authored tolerance against its current executable quote; 100% tolerance never permits zero output.

**System swap guard.** User swaps have no Actors-specific reference guard. System swaps require fresh nonzero directed reference values and MUST satisfy, before mutation and against returned actual amounts when they differ from the quote:

```text
exec_out * ref_in * Perbill::ACCURACY
  >=
(Perbill::ACCURACY - max_deviation) * ref_out * exec_in
```

All products use widened checked arithmetic. A missing, stale, or zero reference or excessive negative deviation is Temporary. The router neither owns nor reinterprets this guard.

**Liquidity.** Ordered LP identity is validated at admission and execution. Add and Donate actual debits MUST stay within supplied caps, and RemoveLiquidity MUST debit the exact resolved LP amount. For `DonateLiquidity`, Actors derives `max_b = preservable_balance(asset_b)` as the cap on pre-existing asset-B debit; the adapter MUST enforce it, and Actors independently verifies the returned asset-A spend.

**Staking.** Staking-share identity is admitted and stable. `Unstake(Percent(100%))` resolves to the full current share balance. Transferable staking receipts remain ordinary custody (§11.3).

**Certified AddressEvent ingress.** Only certified producers create AddressEvent semantics (§5.7):

| Protocol | Ordering and atomicity owner |
| --- | --- |
| `PostMovementNotify` | read-only preflight, movement, one notify; producer storage transaction |
| `BlockAtomicPostDispatch` | read-only preflight, successful dispatch, one notify; block/import state transaction |
| `XcmTransactionalPrecommit` | read-only preflight, Actors precommit, consume/deposit holding; asset-transactor storage transaction |

Every certified protocol defines its source and provenance, read-only preflight, atomicity owner, Actors consequence owner, failure mapping, and complete Weight. Uncertified movement, an absent or Dormant destination, and zero, self, or no-op movement are balance-only. Terminal substitution happens before any funding or readiness consequence (§10.2). Fee collection is never certified ingress.

**Observation publication.** Observation publication creates no Actors ingress, subscription, Pending obligation, or Trigger cause. Its cost and revision semantics belong to the observation owner; Actors read the committed current value only when an authored predicate is evaluated (§7.2).

---

## 13. Calls, events, APIs, and errors

### 13.1 Calls and authorization

Canonical calls:

```text
create_user_actor
create_user_actor_at_slot
create_system_actor
create_system_actor_at_sovereign_id
activate_actor
deactivate_actor
pause_actor
resume_actor
manual_trigger
update_contract
cancel_run
close_actor
set_global_circuit_breaker
set_active_actor_limit
permissionless_sweep
permissionless_sweep_many
actor_prepass
```

| Call | Origin |
| --- | --- |
| User creation | signed creator |
| System creation, locator reuse, active limit | `SystemOrigin` |
| Circuit breaker | configured control origin |
| User control | signed owner, subject to mutability (§10.1) |
| System control | signed owner or `SystemOrigin`, subject to mutability (§10.1) |
| Sweep | any signed origin |
| Prepass | mandatory unsigned inherent origin |

Creation charges the ordinary transaction fee, Actor Creation Fee, and state-hold delta (§8.2). `manual_trigger` follows Manual semantics (§5.7, §8.3). `close_actor` is signed Mutable User control and follows Close (§10.3).

### 13.2 Events and ordering

The ordered event ABI is normative:

```rust
enum Event<AccountId, AssetId, Balance> {
  ActorCreated { actor_id: ActorId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle },
  ActorActivated { actor_id: ActorId },
  ActorDeactivated { actor_id: ActorId },
  ActorPaused { actor_id: ActorId },
  ActorResumed { actor_id: ActorId },
  ActorClosed { actor_id: ActorId, reason: CloseReason },
  CycleStarted { actor_id: ActorId, cycle_nonce: u64 },
  CycleSummary { actor_id: ActorId, cycle_nonce: u64, result: CycleResult, outcomes: OutcomeTotals },
  CycleSuspended { actor_id: ActorId, cycle_nonce: u64, cursor: u32, reason: SuspensionReason, cumulative_outcomes: OutcomeTotals },
  CycleContinued { actor_id: ActorId, cycle_nonce: u64, cursor: u32 },
  CycleCancelled { actor_id: ActorId, cycle_nonce: u64, reason: CancellationReason },
  CycleStopped { actor_id: ActorId, cycle_nonce: u64, step_index: u32 },
  StepSkipped { actor_id: ActorId, cycle_nonce: u64, step_index: u32, reason: StepSkippedReason },
  StepFailed { actor_id: ActorId, cycle_nonce: u64, step_index: u32, retry_class: RetryClass, error: DispatchError },
  TransferExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance, to: AccountId },
  SplitTransferExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, total: Balance, distributed: Balance, retained: Balance, legs: u32, effective_legs: u32 },
  SwapExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance },
  BurnExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  MintExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  StakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance },
  UnstakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, shares: Balance },
  LiquidityDonated { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, max_amount_a: Balance, max_amount_b: Balance, amount_a: Balance, amount_b: Balance },
  LiquidityAdded { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance, lp_minted: Balance },
  LiquidityRemoved { actor_id: ActorId, cycle_nonce: u64, step_index: u32, lp_asset: AssetId, lp_amount: Balance, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance },
  ContractUpdated { actor_id: ActorId },
  ActiveActorLimitSet { old_limit: u32, new_limit: u32 },
  GlobalCircuitBreakerSet { paused: bool },
  ManualTriggerSet { actor_id: ActorId },
  TriggerOccurrenceProcessed { actor_id: ActorId, trigger_family: TriggerFamily, fee: Balance },
  PipelineFeeCharged { actor_id: ActorId, fee: Balance },
  ActionFeeCharged { actor_id: ActorId, cycle_nonce: u64, step_index: u32, actual_effect_weight: Weight, fee: Balance },
  SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 },
  IdleStarvationDetected { consecutive_blocks: u32 },
  IdleStarvationRecovered { consecutive_blocks: u32 },
}
```

Ordering:

1. Useful Trigger fee and latch commit before `TriggerOccurrenceProcessed` (§5.2).
2. Opening emits `PipelineFeeCharged`, then `CycleStarted` (§6.2).
3. A Step event precedes any Cycle boundary event caused by that Step (§7.5).
4. Cancellation emits `CycleCancelled`, then `CycleSummary(Cancelled)` (§6.6).
5. A closing Cycle boundary emits `CycleSummary`, then `ActorClosed` (§10.3); pure Idle close emits only `ActorClosed`.
6. `ActionFeeCharged` is the final receipt of a committed Action-bearing Attempt (§8.5).
7. Fee-collection or enclosing transaction failure emits no rolled-back event.
8. Zero-Step Opening emits `CycleStarted`, then `CycleSummary(Completed)`, then optional `ActorClosed` (§6.5).
9. Redundant latched activity emits no Trigger event (§5.3).
10. Active creation emits `ActorCreated` only; Dormant-to-Active emits `ActorActivated`. Contract replacement or deactivation emits cancellation events first when a run exists, then `ContractUpdated` or `ActorDeactivated` (§6.6, §10.5).

Exact fields and discriminants come from metadata.

### 13.3 Runtime APIs

The runtime MUST expose read-only APIs for:

| Capability | Content |
| --- | --- |
| Simulation | At most one current Step of a fresh plan or the current run, rollback-only, under an explicit synthetic Actor Control and Shared Economic budget |
| Cost quote | Creation fee, prospective Trigger fee, prospective Pipeline Machine fee with its components, maximum next Action fee, and Actor state hold |
| Block resources | Configured budget, current-block meter state, and the last finalized block snapshot |
| Eligibility | Absence, dormancy, or the Active classification (§3.5) with latch and residence placement |

Simulation MUST use the same classifier, precondition, amount, Task, fee, outcome, and policy owners as production (§3.5, §7, §8). A waiting Actor returns not-ready, insufficient synthetic resource returns resource deferral, and fee-collector failure returns its typed error; simulation persists no state or event. No API exposes a remaining Pipeline Machine budget, because none exists (§8.4). Exact request and response types come from metadata.

Clients MUST NOT assemble semantic Contracts from raw storage heads, chunks, or pages; authored Contracts are verified against their semantic Contract ID (§4.2).

### 13.4 Errors and projections

Core classification errors:

```rust
enum ActorClassificationError {
  ActorInvariant,
  RunInvariant,
  ComputationOverflow,
}
```

They project exactly to dispatch, runtime-API, and simulation errors and MUST NOT become absence, dormancy, waiting, or scheduler exhaustion.

```rust
enum Error {
  ActorIdOverflow, ActorNotFound, ActiveActorCapacityExceeded, ActiveActorCountInvariant,
  ActorIdentityCapacityExceeded, ActorIdentityCountInvariant, ActorInvariant, ActorAlreadyActive,
  ActorDormant, ActiveActorLimitExceedsQueueCapacity, ActiveActorLimitTooHigh,
  ActiveActorLimitTooLow, ActiveActorLimitBelowCurrent, ActorPaused,
  ContractStepsExceedOnIdleBudget, ExecutionDelayTooLong, GlobalCircuitBreakerActive,
  ImmutableActor, InsufficientBalance, InsufficientFee, TriggerFeeCollectionFailed,
  InvalidAmountResolution,
  InvalidParkedBalanceActivation, InvalidPredicate, InvalidAutoCloseNonce, InvalidScheduleWindow, InvalidSplitTransfer, InvalidTriggerConfiguration,
  InvalidTradeBound, InvalidRetryAttemptLimit, InvalidObservationMaxAge, SelfTransferNotAllowed,
  MintNotAllowedForUserActor, NotGovernance, NotOwner, OwnerSlotCapacityExceeded,
  OwnerSlotOccupied, InvalidOwnerSlot, ActorIdOccupied, SystemSovereignCapacityExceeded,
  SystemSovereignUnknown, SystemSovereignOccupied, SystemSovereignInvariant,
  SovereignAccountCollision, ReservedSovereignAccount, TooManyContractSteps,
  QueueTicketExhausted, SchedulerIndexExhausted,
  AutoCloseNonceHorizonExceeded, ControlMutationRateLimited, QueueCapacityUnavailable,
  RetryLaterNotAllowedForImmutableActor, ActorRunNotFound, ActorRunInvariant, ComputationOverflow,
  EmptyPrecondition, ManualSourceDisabled, RecipientDepositUnavailable,
  SystemActorTopologyInvalid,
  AdmissionBoundOverflow, StateHoldUnavailable, StateHoldInvariant, StateHoldOverflow,
  PrepassDuplicateOrStale, ResourceProtocolFailed, PrepassContextIncomplete,
}
```

The error surface MUST distinguish absence, dormancy, and active-state mismatch; authorization and mutability; invalid Contract, Trigger, predicate, amount, trade, retry, and window; capacity and bound overflow; slot, locator, collision, and reserved account; state hold and fee collection; Service, Deadline, and detector invariant failure and monotonic exhaustion; control rate limit and circuit breaker; and Prepass, round, and resource protocol failure. Stale admission or body authority projects to `ActorInvariant`, and stale run authority to `ActorRunInvariant`.

```rust
enum CloseReason {
  OwnerInitiated,
  CycleAdmissionInsufficient,
  TriggerAdmissionInsufficient,
  ConsecutiveFailures,
  WindowExpired,
  CycleNonceExhausted,
  AutoCloseNonceReached,
  RetryAttemptsExhausted,
  ProductiveCycleCompleted,
  SchedulerIndexExhausted,
}

enum CancellationReason {
  Explicit,
  ContractReplaced,
  Deactivated,
  Closing(CloseReason),
}
```

Resolution outcomes are not pallet errors. `TaskFailure.error` MAY carry a stable dispatch diagnostic without converting the Attempt into a rejected extrinsic.

---

## 14. Storage, upgrades, configuration, and conformance

### 14.1 Storage and integrity

Generated descriptors own exact keys, hashers, prefixes, values, and page geometry. Normatively, storage MUST hold only the canonical facts of §4.5 and the run of §6.1, keep every collection bounded with exact reverse ownership, hold one residence per serving process plus bounded Trigger deadline authority (§3.1), retain no unbounded execution history, fee cache, remaining Pipeline budget, or generic cache-revalidation workset, and reconcile holds exactly (§8.2).

`try_state` MUST verify Contract, body, and admission bindings, run, latch, residence, detector topology, counters, slots, locators, and orphans. Orphan physical records are never semantic authority and are integrity failures unless they are transaction-local writes that roll back.

### 14.2 Runtime upgrades

Before first production genesis, a fresh canonical baseline MAY replace storage or ABI without migration compatibility. After deployed production lineage, every storage or semantic rewrite MUST define source and target schemas, a bounded migration unit and progress owner, an execution gate, Weight, interruption, resume, and idempotence, Actor and custody disposition, an open-run `Cancel | PreserveWithProof` policy, admission re-certification, and the storage-version transition.

A Weight or fee-policy change may alter admission identity but MUST NOT alter semantic Contract identity (§4.2, §4.4). Open paid Pipelines retain their paid service identity through the Cycle boundary unless the migration proves another disposition. A deployed change MUST NOT make previously reattachable custody unreachable without explicit custody disposition (§11.3).

### 14.3 Runtime configuration

Required relations:

1. `0 < MaxContractSteps <= 255`; each host selects its bounded value.
2. `0 < MaxOwnerSlots <= 255`.
3. `MaxRetryAttempts >= 2`, and `MaxContractSteps * MaxRetryAttempts` fits the outcome counters.
4. Current-Step predicate and amount-read bounds cover every admitted Contract.
5. Every Service, Deadline, detector, sweep, and worker bound is nonzero and owns one complete worst-case unit.
6. One maximum current-Step control and effect transition fits the guaranteed base pass (§9.2).
7. Maximum admitted create, activate, update, deactivate, cancel, and close paths remain dispatchable under their call limits.
8. `MinUserBalance >= host minimum balance` and `ActorCreationFee > 0`.
9. `WeightToFee` maps every nonzero User Trigger, Pipeline, and Action Weight upper bound to a positive fee.
10. `host_control_share` is in `(0, 1]` and the meters split as in §9.1.
11. `MaxTemporalDelayTicks` and `MaxExecutionDelayBlocks` are clock-specific and representable, and the consensus tick is positive.
12. `MaxSplitTransferLegs >= 2`.

The host's integration documentation owns concrete values, page sizes, worker counts, and generated fee-envelope identities.

### 14.4 Conformance

A runtime conforms iff:

1. Metadata exposes only the canonical public shapes of this specification;
2. Every semantic function follows its owning section, and every cross-reference uses rather than redefines it (§1);
3. Every reachable transition is bounded, pre-admitted, and transactionally atomic at its defined boundary;
4. Residence, rounds, wake, parking, and classification follow §3;
5. Contract storage, identity, and admission follow §4;
6. Trigger occurrence, latch, re-arm, underfunding, and detection follow §5;
7. Opening, Attempt inputs, nonce, zero-Step, and Cycle boundaries follow §6;
8. Q1 Step execution, error policies, and Tasks follow §7;
9. Creation, Trigger, Pipeline, Action, and state-hold ownership follow §8 without overlap;
10. Meters, block passes, and liveness follow §9;
11. Mutability, terminal precedence, close, apoptosis, and the circuit breaker follow §10;
12. Sovereign custody survives process deletion and exact reattachment follows §11;
13. Each Task effect maps to one canonical host owner and typed failure surface (§12);
14. Calls, events, APIs, and errors follow §13;
15. Storage integrity and deployed upgrades follow §14.1 and §14.2;
16. Generated Weight upper-bounds every admitted control and effect branch in both dimensions;
17. Simulation is rollback-only and observationally equivalent to zero or one production current-Step transition.

---

_End of specification._
