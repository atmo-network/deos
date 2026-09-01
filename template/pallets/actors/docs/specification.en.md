# DEOS Actors Specification

- **Scope**: Bounded economic actor runtime contract
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase.

This document owns runtime behavior and semantic type meaning. Runtime metadata owns exact SCALE encoding. Generated storage descriptors own exact physical layout. Generated Weight descriptors own measured `Weight(RefTime, ProofSize)`.

Rust blocks that do not declare public types, calls, events, errors, or runtime APIs are normative semantic pseudocode. They define branch order, dependencies, and failure propagation, not source identifiers or Rust ownership syntax.

---

## 1. Authority and sole ownership

### 1.1 Sole-owner rule

Every semantic function has exactly one normative owner in this specification. Other sections MAY use that function only by reference in parentheses, for example `Opening (§5.2)`.

A repeated description MUST NOT redefine behavior. If two passages conflict, the passage in the sole-owner section wins.

### 1.2 Owner registry

| Function or semantic surface | Sole normative owner |
| --- | --- |
| Core determinism, boundedness, Q1, and atomicity | §2 |
| Actor Contract public shape | §3.1 |
| Semantic Contract identity and body commitment | §3.2 |
| C6 hot-head/lazy-tail geometry | §3.3 |
| Admission certificate and Pipeline Machine envelope | §3.4 |
| Canonical partitions and field ownership | §3.5 |
| Trigger families and runtime phase | §4.1 |
| Useful Trigger occurrence | §4.2 |
| Trigger latch and redundant-source behavior | §4.3 |
| Trigger underfunding and source advancement | §4.4 |
| Trigger re-arm | §4.5 |
| Causal cohort | §4.6 |
| Trigger-family timing and crossing semantics | §4.7 |
| Cycle and run state | §5.1 |
| Pipeline Opening | §5.2 |
| Opening and funding snapshots | §5.3 |
| Cycle nonce | §5.4 |
| Zero-Step Cycle | §5.5 |
| Cycle completion and cancellation | §5.6 |
| One-Step attempt transaction | §6.1 |
| Precondition | §6.2 |
| Amount resolution | §6.3 |
| Funding accumulation | §6.4 |
| Step outcome and error-policy interpretation | §6.5 |
| Task semantics | §6.6 |
| Creation fee and state hold | §7.2 |
| Trigger fee | §7.3 |
| Pipeline Machine fee | §7.4 |
| Action fee | §7.5 |
| Actor Control and Shared Economic meters | §7.6 |
| Active Actor classification | §8.1 |
| Placement and temporal readiness | §8.2 |
| Prepass, cutoff, Drain, and FIFO service | §8.3 |
| Detector workers, cohorts, and faults | §8.4 |
| Scheduler liveness | §8.5 |
| Class and mutability | §9.1 |
| Terminal precedence | §9.2 |
| Close cleanup | §9.3 |
| Minimal User apoptosis | §9.4 |
| Control transitions and breaker | §9.5 |
| Sovereign-account derivation | §10.1 |
| User slots and System locators | §10.2 |
| Custody reattachment and recovery | §10.3 |
| Protected minimum | §10.4 |
| Canonical Task-effect ownership | §11.1 |
| Adapter interfaces | §11.2 |
| Failure classification | §11.3 |
| Swap, liquidity, staking, and ingress special rules | §11.4 |
| Calls and authorization | §12.1 |
| Events and ordering | §12.2 |
| Runtime APIs | §12.3 |
| Errors and projections | §12.4 |
| Storage and integrity | §13.1 |
| Runtime upgrades | §13.2 |
| Runtime configuration | §13.3 |
| Conformance | §13.4 |

---

## 2. Core contract

### 2.1 Invariants

1. Equal canonical state and block context MUST produce equal behavior.
2. Every path MUST be `O(1)` or `O(K)` under explicit finite bounds.
3. Complete `Weight(RefTime, ProofSize)` for the current transition MUST fit before semantic mutation.
4. An Actor Contract is a bounded linear sequence of `0..=MaxContractSteps` Steps (§3.1).
5. Contracts contain no loops, jumps, nested contracts, opaque dispatch, Task-authored memory, or authored whole-pipeline rollback.
6. The service quantum is Q1: one Actor MAY commit at most one Step in one block (§6.1).
7. A Pipeline MAY span multiple blocks and is non-atomic across committed Steps (§6.1).
8. An admitted Pipeline MUST NOT undergo economic apoptosis before its Cycle boundary (§9.4).
9. A cause observed in block `N` cannot authorize Actor execution before block `N + 1` (§8.3).
10. User and System Actors share one strict FIFO and class-neutral service order (§8.3).
11. Trigger occurrence, Pipeline Opening, and Action execution are independent transitions (§4.2, §5.2, §6.1).
12. Trigger, Pipeline Machine, and Action work have disjoint economic owners (§7.3-§7.5).
13. Close deletes process semantics and MUST preserve sovereign custody (§9.3, §10.3).
14. Unknown capability, stale authority, unbounded work, invalid actual Weight, or unknown downstream failure MUST fail closed.
15. Rejected control transitions restore Actors state, Actors events, scheduler state, topology, and Actors fee movement to pre-state. Host nonce and ordinary transaction-payment effects are outside this definition.

### 2.2 Atomicity terms

| Term | Meaning |
| --- | --- |
| Rejected control transition | Call returns `Err`; Actors-owned state and events equal pre-state. |
| Provisional Task commit | Task layer succeeded inside the current Step transaction but is not durable until that transaction commits. |
| Committed unsuccessful attempt | Current Step transaction durably commits suspension or failure; fees owned by that committed transition remain charged. |
| Rolled-back scheduler attempt | Current Step transaction fails; queue, run, Task effects, Actors fees, and Actors events equal pre-attempt state. |

---

## 3. Actor Contract and canonical state

### 3.1 Public Contract model

```rust
struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy> {
  trigger: Trigger,
  cooldown_blocks: u32,
  window: Option<ScheduleWindow<BlockNumber>>,
  steps: Steps,
  funding: FundingPolicy,
  completion: CompletionPolicy,
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

`steps.len()` MUST be in `0..=MaxContractSteps`. Zero Steps is first-class (§5.5).

Every semantic Contract replacement replaces the complete authored value. Canonical equality is an exact no-op before rate limiting, cancellation, clocks, topology, writes, fees, or events.

### 3.2 Semantic identity

The semantic Contract ID is the protocol-fixed digest of the admitted canonical authored typed SCALE bytes only:

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
      auto_close_at_cycle_nonce,
    ),
    ordered_steps,
  ))
)
```

The body commitment is independent:

```text
BodyCommitment = Blake2_256(
  SCALE((
    b"DEOS_ACTOR_BODY", // fixed [u8; 15]
    [(0u32, Step[0]), (1u32, Step[1]), ...],
  ))
)
```

Indexes MUST be contiguous and equal the exact Step count. Predicate canonicalization (§6.2) occurs before identity derivation; whitelists are accepted only when already canonical (§4.1). JSON formatting, labels, comments, storage wrappers, Trigger runtime state, run state, tickets, topology, runtime Weight, and physical fragments MUST NOT affect either identity.

### 3.3 C6 physical geometry

One semantic Contract is stored as:

```text
Hot head:
  authored header
  step_count
  first_step: Option<Step>
  SemanticContractId
  BodyCommitment
  admission authority
  fixed-size PipelineMachineEnvelope
  bounded Opening-dependency chunk locator plan
  fixed-size maximum run-state hold quote

Lazy tail:
  contiguous authority-bound chunks
  each containing at most four Steps
  beginning at Step 1
```

Rules:

1. `first_step == None` iff `step_count == 0`.
2. `first_step == Some(Step 0)` iff `step_count > 0`.
3. Zero-Step and one-Step Contracts have no tail chunk.
4. Tail chunks are gap-free, non-overlapping, and keyed by their first covered Step index.
5. Ordinary non-Opening execution loads only the current Step's head or one tail chunk. Opening MAY additionally load only fragments named by the exact Opening dependency plan (§5.3).
6. Unreached Steps MUST NOT add ordinary execution ProofSize merely because they exist; only their explicitly authored Opening dependencies may affect Opening cost (§5.3).
7. `MaxContractSteps` MUST NOT enlarge the maximum current-Step storage read.
8. Full reconstruction is owned only by lifecycle mutation, runtime projection, integrity checks, and upgrade work (§12.3, §13.1, §13.2).

A fragment MUST bind Actor id, semantic Contract ID, body commitment, and exact index range. Missing, stale, overlapping, foreign, or orphan authority fails closed.

### 3.4 Admission certificate and Pipeline Machine envelope

```rust
struct ActorAdmissionCertificate {
  semantic_contract_id: Hash,
  body_commitment: Hash,
  runtime_actor_semantics_version: u32,
  production_weight_identity: Hash,
  body_geometry_version: u32,
  configured_bounds_commitment: Hash,
  admission_identity: Hash,
}

struct ActorStepResourceEnvelope {
  control_weight_upper: Weight,
  effect_weight_upper: Weight,
}

struct PipelineMachineEnvelope {
  service_identity: Hash,
  control_weight_upper: Weight,
}
```

Each authored Step is stored with one runtime-owned `ActorStepResourceEnvelope`. The envelope is bound to the Step index, populated C6 fragment, predicate/read geometry, runtime Weight identity, and configured bounds. It is not authored state.

```text
AdmissionIdentity = Blake2_256(SCALE((
  b"DEOS_ACTOR_ADMISSION", // fixed [u8; 20]
  semantic_contract_id,
  body_commitment,
  runtime_actor_semantics_version,
  production_weight_identity,
  body_geometry_version,
  configured_bounds_commitment,
)))
```

The certificate is runtime-owned derived authority. It MUST bind the current semantic Contract, body, runtime semantics, production Weight identity, C6 geometry, and configured bounds.

`PipelineMachineEnvelope.control_weight_upper` is the generated maximum **reachable** Actor Control work of one complete admitted Pipeline. Its derivation MUST include:

- Opening exactly once (§5.2);
- Each reachable current-Step control path (§6.1);
- Repeatable retry control only up to authored bounds (§6.5);
- At most one reachable finalization or close-control branch (§5.6, §9.3);
- No Trigger work (§4.2);
- No Task-effect work (§11.1);
- No minimal pre-Opening apoptosis (§9.4).

The envelope MUST NOT be the arithmetic sum of mutually exclusive Step maxima. Opening-only work MUST NOT be multiplied by Step 0 retries. Finalization-only work MUST NOT be multiplied across Steps.

Create and semantic update are the sole generation owners. Opening reads the fixed-size envelope in `O(1)` and MUST NOT load tail chunks solely to quote Pipeline service (§5.2, §7.4).

A stale certificate or envelope fails closed until bounded re-certification or authorized Contract replacement. Re-certification MUST preserve semantic identity when authored bytes are unchanged.

### 3.5 Canonical partitions and semantic ownership

The only canonical Actor partitions are:

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

struct ActorIdentity<AccountId, BlockNumber> {
  sovereign_account: AccountId,
  owner: AccountId,
  actor_class: ActorClass,
  mutability: Mutability,
  cycle_nonce: u64,
  last_control_mutation_block: BlockNumber,
}

struct ActorHot<BlockNumber> {
  lifecycle: ActiveLifecycle,
  cycle_state: CycleState,
  trigger_runtime_state: TriggerRuntimeState,
  unsuccessful_attempt_streak: u32,
  pending_signal: bool,
  queue_ticket: Option<QueueTicket>,
  pipeline_wakeup: Option<PipelineWakeupPointer<BlockNumber>>,
  trigger_wakeup: Option<TriggerWakeupPointer>,
  schedule_anchor: BlockNumber,
  last_cycle_block: Option<BlockNumber>,
  terminal_at: Option<BlockNumber>,
}

struct ActorFunding<AssetId, Balance> {
  funding_accumulated: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
}

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

`ActorRunState` is defined only in §5.1.

Semantic owners:

| Fact | Sole canonical owner |
| --- | --- |
| Owner, class, mutability, sovereign account, latest terminated nonce | `ActorIdentity` |
| Authored Contract, identities, Step count, first Step, admission authority, Pipeline envelope | C6 hot head (§3.3) |
| Authored Steps 1..N | C6 tail chunks (§3.3) |
| Lifecycle, cycle phase, Trigger phase, latch, placement pointers, failure streak, clocks | `ActorHot` |
| Funding accumulation and tracked set | `ActorFunding` |
| Open-cycle cursor, snapshots, outcomes, retry state, paid Pipeline authority | `ActorRunState` (§5.1) |
| Physical queue, detector, wakeup, locator, and page authority | derived topology only (§8.2, §8.4, §10.2) |

`ActorType` is derived from `ActorClass` and MUST NOT be stored.

Composite Actor values and runtime API views are read-only and MUST NOT become write models.

Dormant means only `ActorIdentity` and class locator/slot authority exist. Dormant Actors own no hot Contract, hot state, funding, run state, detector membership, ticket, wakeup, or Active-state hold. Public creation admits Dormant only as Mutable; a host genesis configuration MAY declare a sealed Immutable System identity, which can never activate or close through Actor control.

---

## 4. Trigger machine

### 4.1 Trigger families and runtime state

```rust
enum Trigger<AccountId, AssetId, FeedId> {
  Manual,
  AddressEvent {
    source_filter: SourceFilter<AccountId>,
    asset_filter: AssetFilter<AssetId>,
  },
  ObservationChange { feed: FeedId },
  ObservationCrossing {
    feed: FeedId,
    direction: CrossingDirection,
    threshold: u128,
    rearm_threshold: u128,
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
  ObservationCrossing {
    phase: CrossingPhase,
    installed_at_revision: ObservationRevision,
  },
  AtTime {
    anchor_tick: Option<u64>,
    consumed: bool,
  },
  Cadenced {
    anchor_tick: Option<u64>,
  },
}

enum CrossingPhase { Armed, WaitingForRearm }
enum CrossingDirection { Rising, Falling }
```

Compatibility is exact:

- `Manual`, `AddressEvent`, `ObservationChange` require `Stateless`;
- `ObservationCrossing` requires crossing state;
- `AtTime` requires AtTime state;
- `Cadenced` requires cadence state.

Mismatch is `ActorInvariant` (§12.4). Whitelists MUST be nonempty, duplicate-free, and strictly ordered by canonical typed SCALE bytes; runtime admission MUST reject rather than normalize noncanonical input.

### 4.2 Useful Trigger occurrence

A Trigger occurrence is exactly one family-specific cause that:

1. Passes source/detector authority;
2. Is semantically relevant;
3. Observes `pending_signal == false`;
4. Successfully charges the User Trigger fee or is System fee-exempt (§7.3);
5. Commits `pending_signal: false -> true`;
6. Disables further Actor-specific occurrence work until re-arm (§4.3);
7. Emits `TriggerOccurrenceProcessed` (§12.2).

A Trigger occurrence is not a Cycle and does not charge Pipeline service (§5.2, §7.4).

Source publication, movement, or explicit-call work has its own owner (§7.7, §11.4). Trigger occurrence owns only Actor-specific detection/matching/materialization attributable to `false -> true`.

### 4.3 Latch and redundant activity

`pending_signal` is the sole readiness latch.

```text
false -- useful occurrence --> true -- Opening --> false
```

While `pending_signal == true`:

- Source activity MUST NOT create another Actor-specific occurrence;
- No additional Trigger fee is charged;
- No additional Pipeline is queued;
- No Trigger history is accumulated for the Actor;
- No Opening/funding snapshot is changed;
- Where practical, the Actor MUST be absent or disabled in the relevant detector topology.

Source-owned canonical state MAY continue changing. Economic ingress MAY still update custody and funding (§6.4, §11.4).

The latch bounds one Actor to:

```text
at most one open Pipeline
+ at most one deferred Pipeline request
```

There is no direct latch-clear call. Opening consumes it; deactivation or Close deletes it (§5.2, §9.3). Manual occurrence while Running or Suspended requests only the next Cycle and never alters the current cursor, retry, or eligibility (§4.7).

### 4.4 Trigger underfunding and source advancement

User Trigger underfunding means the sovereign account cannot pay the current Trigger fee (§7.3). It MUST NOT create free polling or repeated charging of the same cause.

| Family | Underfunded result |
| --- | --- |
| `Manual` | Reject the call; no Actors state/event/Trigger fee changes. Ordinary signed transaction payment remains. |
| `AddressEvent` | The certified movement and independent funding semantics MAY commit; no latch or Trigger fee. The same movement is not retried as a Trigger. |
| `ObservationChange` | The source revision frontier advances; no latch or Trigger fee. A later revision may form a new cause. |
| `ObservationCrossing` | The transition frontier advances and crossing phase consumes the fire as if no readiness were purchased; no latch or Trigger fee. A later re-arm and later crossing may form a new cause. |
| `AtTime` | The one-shot source is consumed. A User Actor undergoes minimal apoptosis with `TriggerAdmissionInsufficient` (§9.4). System is fee-exempt. |
| `Cadenced` | The due point is skipped and the next future cadence point is installed; no latch or Trigger fee. |

Fee-collector infrastructure failure is not underfunding. It preserves the exact source obligation, records/returns the owning failure, and performs no occurrence mutation (§7.3, §8.4).

### 4.5 Trigger re-arm

Opening consumes the current latch and re-arms the Trigger (§5.2):

| Family | Re-arm rule |
| --- | --- |
| `Manual` | Stateless and enabled. |
| `AddressEvent` | Stateless and enabled. |
| `ObservationChange` | Re-enable from the current authoritative revision; no replay of latched revisions. |
| `ObservationCrossing` | Derive phase from the current authoritative value and revision; no replay of latched transitions. |
| `AtTime` | Never re-arm; `consumed == true`. |
| `Cadenced` | Install the first canonical cadence deadline strictly after the current authoritative tick; no catch-up. |

A Running or Suspended Actor may therefore acquire one new latch for the next Cycle while its current Cycle continues (§5.1).

### 4.6 Causal cohort

A causal cohort is a bounded ordered set of Actors affected by one shared Trigger cause.

Cohort eligibility depends only on shared causal authority, not Contract length, Actor class, Task kind, fee, or Weight.

The cohort MAY amortize:

- Source/detector traversal;
- Candidate classification;
- Useful occurrence materialization (§4.2);
- Grouped physical placement writes (§8.4);
- First Opening/Step-0 control when generated branches remain compatible (§5.2, §6.1).

Every Actor retains its own Trigger fee, latch, ticket, events, Pipeline fee, and Step outcome.

After Step 0 commits, each nonterminal Actor leaves the initial cohort and follows independent Q1 scheduling (§6.1, §8.3). A zero-Step member ends at Opening (§5.5). A one-Step member is the degenerate case whose Step 0 also completes its Cycle.

Unreached pipeline length MUST NOT add first-reaction work merely because it exists. First reaction may load only Step 0 plus exact authored Opening dependencies; unrelated tail fragments remain cold (§3.3, §5.3).

### 4.7 Trigger-specific semantics

#### Manual

Only the authorized owner may call `manual_trigger` (§12.1). A useful call follows Trigger occurrence (§4.2). A latched call is an exact Actor no-op and retains ordinary transaction payment (§7.3).

#### AddressEvent

Only certified positive non-self movement may form an AddressEvent cause (§11.4). Trigger matching is independent from funding acceptance (§6.4). `SourceFilter::Any` accepts any source, including absent source; `OwnerOnly` requires concrete source equal to owner; `Whitelist` requires a concrete listed source. `AssetFilter::Whitelist` requires the exact listed asset.

#### ObservationChange

Every accepted changed feed revision is a source cause. Equality is an exact source no-op. Revision regression fails. Subscriber topology is paged and bounded (§8.4).

#### ObservationCrossing

Rising requires `rearm_threshold < threshold`. Falling requires `rearm_threshold > threshold`.

```text
Rising fire:  previous < threshold && current >= threshold
Rising rearm: previous > rearm_threshold && current <= rearm_threshold

Falling fire:  previous > threshold && current <= threshold
Falling rearm: previous < rearm_threshold && current >= rearm_threshold
```

Repeated equal observations cause neither transition. Installation derives phase from current canonical observation and never retrofires history. Every accepted revision transition is processed in per-feed revision order (§8.4).

#### AtTime

`after_ticks > 0`. The source is one relative consensus-time deadline. It fires at most once, never catches up, and never re-arms (§4.5).

#### Cadenced

`every_ticks > 0`. Cadence is anchored at Active installation using ceiling quantization. A useful occurrence disables cadence detection until Opening re-arms it (§4.5). Missed latched-period points are forgotten.

#### Time bounds

The reference tick is 500 ms. `AtTime` and `Cadenced` disallow `cooldown_blocks` and `ScheduleWindow`. Their delay/period MUST fit `MaxTemporalDelayTicks` (§13.3).

For signal-driven Triggers, block eligibility is owned by:

```text
cooldown_anchor = last_cycle_block.or(schedule_anchor)

cooldown_eligible_at =
  schedule_anchor                                      for the first Cycle
  checked_add(cooldown_anchor, cooldown_blocks)        otherwise

signal_eligible_at(cause_floor) =
  max(cause_floor, cooldown_eligible_at, window.start when present)
```

A ScheduleWindow is inclusive. Validation requires `end > start`, representable `end + 1`, inclusive length `>= MinWindowLength`, current block `<= end`, bounded future start delay, and first possible eligibility `<= end`. `terminal_at = end + 1` (§9.2). Active installation sets `schedule_anchor = max(now, window.start)` when a future window exists, otherwise `now`.

Temporal arithmetic is constant-time:

```text
first_due  = ceil(now_millis / tick_millis) + period
next_due   = first canonical cadence point strictly greater than now_tick
```

Missed cadence points are never iterated or replayed. At genesis, AtTime/Cadenced anchors are uninitialized; the first ordinary temporal service after the timestamp inherent installs the full future deadline without creating readiness.

---

## 5. Pipeline and Cycle

### 5.1 Cycle and run state

```rust
struct ActorRunState<BlockNumber, AssetId, Balance> {
  semantic_contract_id: Hash,
  body_commitment: Hash,
  admission_identity: Hash,
  pipeline_service_identity: Hash,
  cycle_nonce: u64,
  cursor: u32,
  opening_predicate_cursor: u32,
  unsuccessful_attempts_at_cursor: u32,
  last_attempt_block: BlockNumber,
  last_committed_step_block: Option<BlockNumber>,
  eligible_at: BlockNumber,
  opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxOpeningSnapshotEntries>,
  opening_predicate_results: BoundedVec<Result<bool, PredicateError>, MaxOpeningPredicateResults>,
  funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  cumulative_outcomes: OutcomeTotals,
  last_step_outcome: Option<StepOutcome>,
  suspension: Option<SuspensionReason>,
}
```

`ActorRunState` exists iff `CycleState` is `Running` or `Suspended`.

- `Running` requires `suspension == None`.
- `Suspended` requires `suspension == Some(_)`.
- `Idle` requires no run.
- `cursor` is the sole current Step index.
- The run binds the exact semantic Contract, body, admission, paid Pipeline service, and open-cycle nonce.
- Completed-run history is not retained in consensus state.

### 5.2 Opening

Opening is the sole transition that consumes paid pending readiness into one Cycle.

Opening MUST execute in this order:

1. Validate current Actor, latch, placement, Contract, admission, and Pipeline envelope (§3.4, §8.1, §8.2).
2. Apply Opening-specific terminal checks (§9.2).
3. Validate that the active-installed maximum run-state hold remains present (§7.2).
4. Check and charge the complete Pipeline Machine fee (§7.4).
5. Preserve the existing run-state hold without a Cycle-local hold mutation (§7.2).
6. Derive `run.cycle_nonce = identity.cycle_nonce + 1` without changing `identity.cycle_nonce` (§5.4).
7. Capture Opening and funding snapshots (§5.3).
8. Consume `pending_signal` and the current funding accumulator (§4.3, §6.4).
9. Re-arm the Trigger (§4.5).
10. Emit `CycleStarted` (§12.2).
11. For a nonempty Contract, execute Step 0 in the same current-Step transaction (§6.1).
12. For a zero-Step Contract, finalize atomically (§5.5).

If the Actor cannot pay the Pipeline Machine fee, or its active-installed run-state hold authority is inconsistent, Opening MUST NOT partially occur. Insufficient Pipeline payment invokes minimal apoptosis with `CycleAdmissionInsufficient` (§9.4); inconsistent hold authority fails closed as an Actor invariant. The prior Trigger fee remains final (§7.3).

If Pipeline fee collection fails despite valid capacity, the entire Opening attempt rolls back, the latch remains consumable, and the live FIFO head is preserved (§7.4, §8.3).

### 5.3 Opening and funding snapshots

Create/update derive one body-commitment-bound `OpeningDependencyPlan` containing only exact fragment locators and counts for authored `Opening` predicates and `PercentageAtOpening` surfaces. It is derived authority, not a second semantic owner. Opening loads only those named fragments and rejects any locator/body mismatch. A Contract with no Opening dependency loads none.

After the Pipeline fee debit and before any Step-0 Task effect, Opening captures once:

```text
opening_snapshot = every unique OpeningSurface referenced by any Step
funding_snapshot = funding_accumulated before its consumption
opening_predicate_results = every admitted Opening predicate result
```

Opening balances are read after the Pipeline fee debit and before any transient Action-fee debit. Each later exact debit independently preserves its own current Action-fee reservation (§7.5, §10.4).

Every admitted Opening surface exists in the snapshot even when zero. Missing admitted keys are invariant failures.

Opening facts are immutable until Cycle termination. Later Steps MAY load only snapshot keys/results referenced by the current Step, but MUST NOT recapture, prune, or rewrite them. `opening_predicate_cursor` maps the current Step to its exact frozen predicate-result range; advance adds that Step's admitted Opening-predicate count, and retry preserves the cursor.

Funding accepted after Opening belongs to the next Cycle and cannot repair the current funding snapshot (§6.4).

### 5.4 Cycle nonce

`ActorIdentity.cycle_nonce` is the latest terminated Cycle nonce.

Opening derives:

```text
run.cycle_nonce = checked_add(identity.cycle_nonce, 1)
```

Opening does not mutate the identity nonce. Final completion, failure, cancellation, or close assigns `identity.cycle_nonce = run.cycle_nonce` exactly once. Retry, suspension, Step progress, breaker deferral, and congestion do not change either nonce.

### 5.5 Zero-Step Cycle

A zero-Step Contract has no Task, Step event, Step fragment, Opening surface, or `ActorRunState`.

A ready zero-Step Cycle consumes one FIFO service opportunity and atomically:

1. Performs Opening (§5.2);
2. Charges the zero-Step Pipeline Machine fee (§7.4);
3. Emits `CycleStarted`;
4. Commits the new Cycle nonce (§5.4);
5. Emits `CycleSummary(Completed)`;
6. Applies terminal policy (§9.2).

`auto_close_at_cycle_nonce = Some(1)` makes a fresh zero-Step Contract one-shot.

### 5.6 Completion and cancellation

Cycle completion occurs at plan end or successful `StopCycle` (§6.6).

Finalization MUST:

1. Record the final Step outcome when present (§6.5);
2. Emit the required boundary events (§12.2);
3. Commit the run nonce (§5.4);
4. Delete `ActorRunState` while preserving the active-installed run-state hold (§7.2);
5. Update `last_cycle_block`;
6. Apply terminal precedence (§9.2);
7. Otherwise return to `Idle`;
8. If `pending_signal == true`, create one next-cycle obligation eligible no earlier than the next block (§8.2).

Cancellation deletes the run without compensating or rolling back earlier committed Steps. It emits `CycleCancelled`, then `CycleSummary(Cancelled)`, commits the run nonce, and deletes the run. Contract replacement, deactivation, close, and incompatible upgrade own their cancellation reason (§9.3, §9.5, §13.2).

---

## 6. Step execution

### 6.1 One-Step attempt transaction

An Attempt is one admitted execution of exactly one current Step. A retry is a later Attempt at the same cursor.

Before reading any `Current` predicate or dynamic amount, the scheduler MUST reserve:

- The exact current Step's maximum Actor Control Weight (§7.6);
- The exact current Step's maximum Task-effect Weight (§7.6);
- For User Actors, the maximum current Action fee (§7.5).

If any reservation does not fit, the Attempt defers without semantic evaluation or mutation.

One current-Step transaction atomically owns:

```text
run/ticket/Contract validation
+ optional Opening for cursor 0
+ current-Step Precondition
+ amount resolution
+ at most one Task effect
+ Action fee settlement when a Task is invoked
+ StepOutcome and counters
+ cursor, suspension, completion, or close transition
+ successor or retry placement
+ Actors events
```

A Task MUST NOT be split across blocks. Earlier committed Steps are never rolled back by later failure.

After a committed Step, `last_committed_step_block` equals the current block. A second Step commit for the same Actor in that block MUST fail closed independently of ticket correctness.

An advancing Step with a successor sets `eligible_at = current_block + 1` and creates one future obligation (§8.2). It does not load or execute the successor Step.

### 6.2 Precondition

```rust
enum ObservationTiming { Opening, Current }

struct TimedPredicate<P> {
  timing: ObservationTiming,
  predicate: P,
}

struct Precondition<P, MaxClauses, MaxPerClause> {
  clauses: BoundedVec<BoundedVec<TimedPredicate<P>, MaxPerClause>, MaxClauses>,
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

A present Precondition is nonempty DNF: outer clauses are OR; inner predicates are AND.

Admission canonicalizes predicates and clauses by canonical typed SCALE order, rejects duplicate clauses after predicate deduplication, and absorbs exact supersets under `A OR (A AND B) = A`.

Every admitted predicate is evaluated; there is no short-circuit because Weight MUST be data-independent.

`Opening` predicates are evaluated once during Opening and frozen (§5.3). `Current` predicates are evaluated immediately before the owning Step and observe prior committed Steps plus intervening external state.

Predicate error is Permanent Step failure. False emits `StepSkipped(PreconditionFalse)` and advances (§6.5).

`Above` is strict `>` and `Below` is strict `<`; equality satisfies neither. Observation `Unavailable`, `Uninitialized`, or `Stale` evaluates false. Structurally invalid `Fresh` observation is `PredicateError::InvalidObservation` and Permanent. `max_age_blocks` MUST be positive. Balance predicates read only the Actor sovereign account, subtract the current transient Action-fee reservation for the fee-native asset, and do not grant spending authority (§7.5, §10.4).

### 6.3 Amount resolution

```rust
enum AmountResolution<Balance> {
  Fixed(Balance),
  PercentageOfCurrent(Perbill),
  PercentageAtOpening(Perbill),
  PercentageOfLastFunding(Perbill),
  AllAvailable,
}

enum OpeningSurface<AssetId> {
  PreservableAsset(AssetId),
  TargetAsset(AssetId),
  StakingShares(AssetId),
}
```

Each Task field resolves to exactly one of:

```text
Resolved(positive value)
Skipped
FundingUnavailable
```

Rules:

- Percentages use widened floor division;
- Dynamic zero is `Skipped`;
- Absent/zero last-funding basis is `FundingUnavailable`;
- A positive exact debit above current capacity is `FundingUnavailable`;
- Missing admitted Opening state is an invariant failure;
- A positive exact amount MUST NOT be silently reduced;
- `AllAvailable` is an ordinary authored amount mode with no lifecycle privilege.

For multiple amount fields:

```text
any FundingUnavailable -> FundingUnavailable
else any Skipped       -> Skipped
else                   -> Executable(all values)
```

Source-capacity calculations preserve the current Action-fee reservation and protected minimum (§7.5, §10.4).

| Resolution surface | Tasks | Current/Opening basis |
| --- | --- | --- |
| Preserve-source | Transfer, SplitTransfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | current preservable balance / `OpeningSurface::PreservableAsset` |
| Output-target | Mint, SwapOut | current spendable target / `OpeningSurface::TargetAsset`; `AllAvailable` forbidden |
| Share-spend | Unstake | current staking shares / `OpeningSurface::StakingShares` |

Fixed, Opening, and last-funding source/share values MUST fit current capacity. Output-target values are bounded by their own authored/adaptor rules, not current target balance. `PercentageAtOpening` never reads Trigger payload.

### 6.4 Funding accumulation

```rust
enum FundingSourcePolicy<AccountId> {
  OwnerOnly,
  SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>),
  RuntimePolicy,
  AnyVerifiedIngress,
}
```

Contract admission derives the exact bounded `funding_tracked_assets` set from all `PercentageOfLastFunding` references, including the canonical staking-share asset mapped for `Unstake`.

A positive certified credit is accumulated only when:

1. Its asset is tracked; and
2. The funding policy accepts source and provenance.

Policy acceptance is exact:

| Policy | Accepted source |
| --- | --- |
| `OwnerOnly` | signed provenance and concrete source equal to owner |
| `SignedAllowlist` | signed provenance and concrete allowlisted source |
| `RuntimePolicy` | configured authority accepts the exact source/provenance pair; all-`None` is denied |
| `AnyVerifiedIngress` | concrete source or typed provenance exists; all-`None` is denied |

Rejected or untracked credit remains custody only. Trigger matching is independent (§4.7).

Opening snapshots and clears the accumulator (§5.2, §5.3). Later funding belongs to the next Cycle. Close or deactivation deletes the accumulator but does not move custody (§9.3).

### 6.5 Step outcome and error policy

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

`RetryLater.max_attempts` counts the first unsuccessful execution and all retries. It MUST be in `2..=MaxRetryAttempts`.

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

A step increments at most one primary outcome counter.

A Retry suspension increments cursor-local and global unsuccessful counters exactly once. Local bound wins when local and global bounds are reached by the same Attempt.

```text
next_local = previous suspension at same cursor ? previous + 1 : 1
next_global = unsuccessful_attempt_streak + 1
backoff(i) = min(2^i, 8 blocks)
```

`FundingUnavailable` is not a Task failure. `AbortCycle` aborts on Task failure, not unavailable funding.

Outcome deltas are exact: precondition skip increments `precondition_skips`; resolution skip increments `skipped_resolution`; advancing funding skip increments `skipped_funding_unavailable`; successful effect increments `executed_steps` and `committed_effectful_tasks`; successful `StopCycle` increments `executed_steps`; invoked Task failure increments `failed_steps`. No other counter changes.

A terminal Failed Attempt increments the global unsuccessful streak once. A `ContinueNextStep` failure does not. A completed Cycle resets the global unsuccessful streak. Deferral, pause, cancellation, exact no-op, and advancing funding skip do not increment it.

Retry eligibility is:

```text
retry_eligible_at = last_attempt_block + max(cooldown_blocks, backoff(next_local - 1))
```

### 6.6 Task semantics

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

General rules:

- At most one Task effect is invoked per Attempt (§6.1);
- `Mint` is System-only;
- Self-transfer and duplicate split recipients are invalid;
- Each Task contains at most two `AmountResolution` fields;
- Every debit preserves the protected minimum (§10.4);
- `Transfer(AllAvailable)` has no close-specific privilege;
- `CloseAfterProductiveCycle` observes only committed effectful Tasks (§9.2);
- Task effects use canonical host operations (§11.1).

`SplitTransfer` requires 2..=`MaxSplitTransferLegs`, positive unique shares, and total share `<= 1`. Each leg is floored; rounding dust remains with the Actor.

`SwapIn` and `SwapOut` use current executable quotes inside the adapter boundary (§11.4). `InputLimit::Absolute` is a cap, not an admission gate.

`AddLiquidity` amounts are debit caps; actual used amounts and LP output are returned. `RemoveLiquidity` debits the exact resolved LP amount. `DonateLiquidity` uses one authored cap plus one current derived preservable cap (§11.4).

---

## 7. Economics and block resources

### 7.1 Economic surfaces

Actors has four service-fee boundaries plus one refundable state resource:

```text
committed User creation        -> ActorCreationFee
useful Trigger occurrence      -> TriggerFee
Pipeline Opening               -> PipelineMachineFee
invoked Action attempt         -> ActionExecutionFee
retained User Actor state      -> ActorStateHold
```

Ordinary transaction payment remains owned by the host transaction-payment contract. Task-native protocol fees remain owned by their underlying mechanism (§11.1).

System Actors are exempt from Actors Creation, Trigger, Pipeline, and Action fees, but consume identical block resources. Task-native protocol fees MAY still apply.

### 7.2 Creation fee and state hold

`ActorCreationFee` is a fixed non-refundable process-admission and anti-spam charge. It is paid by the signed creator only when User creation commits.

It MUST NOT be described as a second payment for create-call Weight. The ordinary transaction fee owns synchronous call computation.

The Creation Fee also economically backs the protocol obligation to perform minimal pre-Opening apoptosis if later required (§9.4). It does not prepay Trigger, Pipeline, or Action service.

`ActorStateHold` is a refundable geometry-backed hold on the User owner account, not a fee and not rent.

Rules:

- Elapsed time does not change it;
- No recurring collection exists;
- Active identity/head/body/detector/funding state is held by actual retained geometry;
- Zero/one-Step Contracts MUST NOT reserve a maximum-size body footprint;
- Active installation reserves one type-derived maximum admitted run-state hold before autonomous Trigger service becomes possible;
- The maximum is derived from bounded runtime types and MUST NOT use a hand-maintained byte constant;
- Opening, Q1 progress, retry, suspension, Cycle boundary, and cancellation MUST NOT mutate or release the run-state hold;
- Deactivation or Close releases the active-installed run-state hold atomically with removal of Active authority;
- State transitions reserve/release all other exact hold deltas atomically;
- Inability to reserve the active-installed run hold rejects creation, activation, or replacement atomically before autonomous service is admitted;
- Close releases all Actor-state hold (§9.3);
- Sovereign custody is not Actor state and is not held by this mechanism (§10.3).

Deactivation releases Active Contract/topology/funding/run holds and retains only the Dormant identity hold (§3.5, §9.5). System state remains host-owned bounded capacity and MAY be hold-exempt, but exemption grants no resource or scheduler preference.

Generated state descriptors own exact byte accounting. The fixed Creation Fee is not a cleanup escrow: minimal apoptosis remains a protocol obligation even if later cleanup cost exceeds the historical fee (§9.4).

### 7.3 Trigger fee

```text
TriggerFee = WeightToFee(maximum generated control Weight of one useful family-specific occurrence)
```

A User Trigger fee is charged exactly once from fee-native balance above `MinUserBalance` before `pending_signal: false -> true` (§4.2, §10.4). Redundant latched-period activity performs no Actor-specific Trigger work and charges no Trigger fee (§4.3).

A useful User `manual_trigger` call charges the complete generated Manual Trigger owner to the sovereign account and returns `Pays::No`. A redundant, rejected, underfunded, or System Manual call retains ordinary transaction payment and charges no User Trigger fee. This is the complete no-double-charge rule for Manual.

Automatic source ingress/publishers pay their own source work; the Actor pays only its occurrence materialization (§7.7, §11.4).

Insufficient Trigger capacity follows the family table (§4.4). Trigger-fee collection failure rolls back occurrence materialization and preserves the exact retry/fault authority (§4.4, §8.4).

Committed Trigger fees are non-refundable and independent from later Pipeline admission.

### 7.4 Pipeline Machine fee

```text
PipelineMachineFee = WeightToFee(PipelineMachineEnvelope.control_weight_upper)
```

The envelope is defined in §3.4.

The User fee is charged once from fee-native balance above `MinUserBalance` at Opening (§5.2, §10.4). It prepays all reachable Actor Control work of that Pipeline through its Cycle boundary, including reachable completion/close control. There is:

- No separate `CleanupFee`;
- No per-Step machine charge;
- No remaining machine-budget ledger;
- No hold/refund settlement;
- No Trigger work in the quote;
- No Task-effect work in the quote.

Running and Suspended Attempts consume paid machine authority without a balance predicate.

Insufficient current capacity causes minimal apoptosis before Opening (§9.4). Fee-collector infrastructure failure rolls back Opening and preserves the latch (§5.2).

Committed Pipeline fees are non-refundable. Every Actors fee transfer is ledger-only: it MUST NOT create AddressEvent ingress, funding accumulation, Trigger readiness, or placement (§11.4).

### 7.5 Action fee

Before an Action-bearing Task is invoked, the current Attempt reserves the maximum Task-effect fee from the User Actor's fee-native balance above `MinUserBalance` (§10.4).

```text
cannot reserve -> FundingUnavailable; no Action invocation; no Action fee
can reserve    -> invoke canonical effect
                  -> settle valid actual effect Weight
                  -> charge actual Action fee on success or typed failure
```

A retry is a new independently charged Action attempt. Before a Suspended User Actor receives retry execution, the scheduler proves that its fee-native balance above `MinUserBalance` can cover the current Action maximum. The Step effect and actual-fee settlement remain one transaction, so unused maximum liability is released and no terminal consensus debt can survive rollback. Insufficient retry liability selects fee-free custody-neutral minimal apoptosis before invocation.

False Precondition, skipped resolution, `FundingUnavailable`, and `StopCycle` invoke no Action and charge no Action fee.

Invalid or greater-than-reserved actual Weight rolls back the current-Step transaction and fails closed.

### 7.6 Actor Control and Shared Economic meters

The production block policy is component-wise:

```text
Actor Control hard ceiling = floor(SchedulableBlockWeight / 3)
Shared Economic            = SchedulableBlockWeight - Actor Control
Actor base turn            = floor(Shared Economic / 2)
User base turn             = Shared Economic - Actor base turn
```

One third is the maximum permissible Actor Control share. Further throughput work MUST optimize inside that envelope and MUST NOT take additional guaranteed capacity from Actor effects or user dispatch. Deterministic floor/remainder ownership prevents resource loss in either Weight component.

Actor Control owns:

- Detector and Trigger occurrence work;
- Latch and placement;
- Pipeline Opening;
- Precondition and amount resolution;
- Run persistence, retry, completion, and close control;
- Scheduler and bounded cleanup.

Shared Economic owns:

- Ordinary external economic calls;
- Actor Task effects.

A Task effect and its external equivalent use the same canonical host mechanism and effect Weight (§11.1).

After mandatory Actor work is accounted, an explicit deterministic host policy MAY lend unused Actor Control capacity to Shared Economic work. Actor Control MUST NOT exceed its one-third ceiling or borrow Shared Economic capacity. Lending MUST NOT change FIFO, causal cutoff, or Q1.

### 7.7 No double charging

No two economic surfaces may charge the same Weight or retained byte.

| Work | Sole economic owner |
| --- | --- |
| External call dispatch/origin work | ordinary transaction fee |
| User process admission | Actor Creation Fee (§7.2) |
| Retained Actor state | Actor State Hold (§7.2) |
| Useful readiness `false -> true` | Trigger Fee (§7.3) |
| Complete admitted Pipeline control | Pipeline Machine Fee (§7.4) |
| Invoked Task effect | Action Fee (§7.5) |
| Router trading fee, staking fee, etc. | underlying host mechanism (§11.1) |

---

## 8. Scheduler and readiness

### 8.1 Active Actor classification

One pure classifier owns structural Active-state classification.

```rust
enum ActorExecutionPhase<BlockNumber> {
  GlobalCircuitBreaker,
  Paused,
  WaitingNextBlock(BlockNumber),
  WaitingRetry(BlockNumber),
  WaitingCadenceTick(u64),
  WaitingSignal,
  Ready,
}

struct ActorClassification<BlockNumber> {
  terminal_reason: Option<CloseReason>,
  execution_phase: ActorExecutionPhase<BlockNumber>,
}
```

The classifier:

1. validates canonical Active partitions (§3.5);
2. validates `CycleState` against `ActorRunState` (§5.1);
3. validates current Contract/run/admission bindings (§3.4, §5.1);
4. computes stored-state terminal predicates (§9.2);
5. computes phase in this order: breaker, pause, terminal-ready, next-block/retry/tick wait, signal wait, ready.

Opening-specific fee and nonce checks are not classifier-owned; they belong to Opening (§5.2).

Classification errors are typed and MUST NOT become absence, waiting, or Ready (§12.4).

### 8.2 Placement and temporal readiness

An Actor owns at most:

- One live FIFO ticket; and
- One pipeline temporal target; and
- One independent Trigger temporal target for `AtTime`/`Cadenced`.

```rust
type QueueTicket = u64;
type WakeupPageId = u64;
type WakeupSlot = u32;
type ObservationRevision = u64;

enum WakeupKey<BlockNumber> { Block(BlockNumber), Tick(u64) }

struct PipelineWakeupPointer<BlockNumber> {
  key: WakeupKey<BlockNumber>,
  page_id: WakeupPageId,
  slot: WakeupSlot,
}

struct TriggerWakeupPointer {
  tick: u64,
  page_id: WakeupPageId,
  slot: WakeupSlot,
}

struct ActorStepTicket<BlockNumber> {
  actor_id: ActorId,
  cycle_nonce: u64,
  cursor: u32,
  ticket: QueueTicket,
  eligible_at: BlockNumber,
  semantic_contract_id: Hash,
  body_commitment: Hash,
}
```

Future work remains in bounded wakeup topology until eligible. The ready FIFO contains only eligible work.

A successor, retry, or retained next-cycle request becomes eligible no earlier than the following block (§6.1, §5.6). Queue pressure preserves the exact obligation; it does not lose readiness or report completion.

An Idle Opening ticket binds the checked prospective nonce `identity.cycle_nonce + 1`; a Running/Suspended ticket binds `run.cycle_nonce`. Ticket and wakeup authority are exact. Mismatch creates a stale tombstone with no semantic authority.

### 8.3 Prepass, cutoff, Drain, and FIFO service

Every block contains exactly one mandatory Actor Prepass after required context inherents and before ordinary external extrinsics.

Prepass:

1. Services bounded stale cleanup;
2. Services only detector/temporal obligations caused before the current block boundary;
3. Materializes eligible readiness (§4.2, §8.4);
4. Freezes one immutable maximum ticket cutoff;
5. Executes the Actor base pass in strict FIFO order.

After ordinary external dispatch, Actors Drain MAY continue service from the same FIFO head using the same cutoff and actual remaining meters (§7.6).

Tickets created after cutoff MUST NOT execute in the current block. Current-block source activity is next-block work. This prevents same-block Actor recursion.

After bounded stale-head cleanup, the valid live FIFO head is authoritative:

- It cannot be bypassed;
- No cheap/heavy/System/User reordering exists;
- If its complete current transition does not fit, the Actor pass stops;
- Later tickets remain untouched.

A maximum valid Step may be the only Actor Step in a block. This is paid bounded service, not structural starvation. Pipeline- or Action-fee collector failure rolls back the current transition, preserves the live head, and stops the pass; it is not a Task failure (§7.4, §7.5).

### 8.4 Detector workers, cohorts, and faults

Detector geometry is source-specific:

```text
Manual              -> direct call
AddressEvent        -> certified destination/filter path
ObservationChange   -> paged broad subscribers
ObservationCrossing -> ordered sparse threshold index
AtTime/Cadenced     -> temporal index
```

A universal Trigger index is forbidden.

Workers MUST:

- Preserve source revision/time order;
- Inspect only exact affected candidates;
- Admit complete multidimensional Weight before mutation;
- Use bounded candidate/page/chunk counts;
- Preserve one exact source frontier on fault;
- Never skip corruption or let later source work overtake it.

A homogeneous causal cohort (§4.6) MAY aggregate physical writes only when every included candidate follows the same generated control branch and compatible destination shape. Cohorting MUST NOT reorder candidates or create Task-shape affinity.

A worker fault records one bounded current fault. Repeated observation of the same uncleared fault emits no duplicate first-recorded event. Repair is explicit and bounded (§12.1, §12.2). The reference profile MUST admit a homogeneous causal cohort of at least 128 candidates when its generated Actor Control Weight and destination capacity fit.

### 8.5 Scheduler liveness

Given:

- A finite pre-cutoff ticket set;
- Recurring conforming Actor Control and Shared Economic capacity;
- Finite stale churn;
- Eventual placement capacity;
- No structural invariant fault;

all live tickets receive service in FIFO order.

The protocol promises no fixed block latency. Increasing runnable population MAY increase inter-Step service gaps while preserving order and eventual service.

Starvation telemetry MUST NOT change priority, order, or execution authority.

---

## 9. Lifecycle

### 9.1 Class and mutability

User creation is signed; the caller becomes owner and consumes one owner slot (§10.2). System creation requires `SystemOrigin` and consumes a System locator (§10.2).

Actor-scoped authorization:

| Actor | Authorized control |
| --- | --- |
| User | signed owner |
| System | signed owner or `SystemOrigin` |

User Mutable MAY update Contract, pause/resume, cancel a run, deactivate, reactivate, and explicitly close.

Public Dormant creation requires `Mutable`. Public Immutable creation MUST install an Active Contract because no later activation authority exists. A host genesis configuration MAY declare an Immutable Dormant System identity as a permanently sealed no-Contract role; activation and owner close MUST reject it as Immutable.

User Immutable:

- Rejects Contract replacement, pause/resume, activation/deactivation, cancellation, and owner close;
- MAY use an authored Manual Trigger;
- Cannot author `RetryLater`;
- An authored `AtTime` Contract MUST set `auto_close_at_cycle_nonce = Some(1)`, so its consumed one-shot source cannot leave an owner-inaccessible inert process;
- May terminate only through authored/lifecycle terminal rules, Trigger-underfunded one-shot apoptosis (§4.4, §9.4), Pipeline-admission apoptosis (§9.4), or deployed-lineage upgrade (§13.2).

System Immutable:

- Rejects actor-scoped control;
- Cannot author Manual or `RetryLater`;
- Is Actors-fee-exempt;
- Remains Active until an independently owned terminal rule or upgrade removes it.

Immutability fixes policy and owner control. It does not freeze runtime Weight, fee conversion, adapter behavior, or host economics. `auto_close_at_cycle_nonce` is changed only by complete Mutable Contract replacement and MUST be strictly above the current terminated nonce within `MaxAutoCloseNonceHorizon`.

### 9.2 Terminal precedence

Terminal predicates are evaluated only by their owning transition.

#### Stored-state classifier precedence (§8.1)

1. `WindowExpired`;
2. `RetryAttemptsExhausted`;
3. `ConsecutiveFailures`;
4. `AutoCloseNonceReached` while Idle;
5. Already-materialized permanent `SchedulerIndexExhausted`.

#### Opening precedence (§5.2)

1. Stored-state terminal reason from classification (§8.1);
2. `CycleNonceExhausted`;
3. `CycleAdmissionInsufficient` when the Pipeline Machine fee cannot be provided (§7.4); active-installed run-hold inconsistency fails as an Actor invariant (§7.2).

#### Step-finalization precedence (§6.5)

1. Retry-local bound;
2. Global failure bound;
3. `ProductiveCycleCompleted` after complete productive Cycle;
4. `AutoCloseNonceReached` after non-suspended Cycle boundary;
5. Permanent successor-placement `SchedulerIndexExhausted`.

#### Trigger-worker terminal (§4.4)

A due User `AtTime` source that cannot pay its Trigger fee selects `TriggerAdmissionInsufficient` and minimal apoptosis (§9.4).

`CloseAfterProductiveCycle` closes only after the complete Cycle reaches `Completed` and `committed_effectful_tasks > 0` (§5.6, §6.5).

### 9.3 Close and cancellation cleanup

Close is lifecycle cleanup only.

It MUST atomically:

1. Cancel any open run (§5.6);
2. Revoke execution authority;
3. Remove hot Contract head, tail chunks, payloads, admission certificate, hot state, funding state, run state, detector memberships, tickets, wakeups, reverse indexes, and holds;
4. Release the User slot or mark the System locator vacant (§10.2);
5. Emit `ActorClosed` (§12.2).

Close MUST NOT:

- Enumerate assets;
- Transfer balances;
- Unwind positions;
- Invoke a Task;
- Select a refund recipient;
- Alter sovereign custody.

Explicit User close is available only to the signed owner of a Mutable User Actor (§12.1). Its ordinary transaction fee is paid by the caller and does not require Actor solvency.

### 9.4 Minimal User apoptosis

Minimal User apoptosis is automatic process-state garbage collection with no custody effect.

It occurs only for:

- `TriggerAdmissionInsufficient` from one-shot User `AtTime` (§4.4);
- `CycleAdmissionInsufficient` before Opening (§5.2); or
- `CycleAdmissionInsufficient` before a Suspended User retry whose current maximum Action liability is unavailable (§7.5).

It MUST NOT occur after an Action effect or leave fee debt. Trigger and Pipeline funding never close a Running or Suspended Pipeline; only the bounded pre-invocation retry-liability rule may close a Suspended User Actor.

Minimal apoptosis performs Close (§9.3) without a Task, custody scan, fee reserve, or economic policy. Its protocol cleanup obligation is economically backed by the committed Actor Creation Fee (§7.2).

There is no `ActorFundingWait` and no automatic `Transfer(AllAvailable)`.

### 9.5 Control transitions and breaker

Semantic Contract replacement, deactivation, explicit run cancellation, close, and incompatible upgrade MUST cancel the open run before changed meaning becomes executable (§5.6).

Pause preserves Contract, run, snapshots, latch, clocks, failure state, and Trigger detector evolution, but removes ordinary Pipeline execution placement. Resume reconstructs the exact required placement (§8.2). A Trigger may therefore latch one next-Cycle request while the Actor is Paused, but no Step executes.

Active creation or activation installs a new Active epoch with `Active/Idle`, no run, `pending_signal = false`, zero failure streak, empty funding accumulator, Contract-derived tracked assets, current Trigger runtime state, schedule clocks, terminal marker, topology, and exact state hold (§3.5, §4.1, §7.2). Activation preserves the Dormant identity nonce. Deactivation cancels any run and removes the complete Active epoch while preserving identity, locator/slot, nonce, custody, and persistent control clock (§5.6, §10.3).

Authorized Mutable control MAY repair a non-window stored terminal condition before scheduler or sweep commits Close. `WindowExpired` substitutes Close before the requested mutation (§9.2).

Mutable control transitions are limited to one committed semantic mutation per Actor per block. Exact no-op returns before rate limiting.

While the global breaker is active:

- FIFO Step effects and ordinary automatic terminal close do not run;
- Mandatory minimal apoptosis (§9.4), explicit close, and bounded sweep cleanup MAY run because they invoke no economic Task;
- Bounded detector, wakeup, stale-cleanup, and fault work MAY continue;
- New Active creation and activation fail;
- Authorized control over existing Mutable Actors remains available;
- Explicit close and permissionless sweep of independently owned terminal state remain available;
- FIFO order and existing placement remain unchanged.

Permissionless sweep is bounded and closes only stored-state terminal reasons (§9.2). It never predicts Trigger or Pipeline affordability. Actor-targeting control and certified ingress may substitute Close only for `WindowExpired`; all other terminal reasons remain scheduler/sweep owned. Dormant activation with exhausted Cycle nonce closes the identity with `CycleNonceExhausted` rather than installing an unusable Active epoch.

---

## 10. Sovereign custody and identifiers

### 10.1 Sovereign-account derivation

```text
User seed   = Blake2_256(SCALE(ActorsPalletId, b"user", owner, owner_slot))
System seed = Blake2_256(SCALE(ActorsPalletId, b"system", system_sovereign_id))
account     = HostSovereignAccountDeriver(seed)
```

Derivation MUST be total, deterministic, class-separated, and stable for every previously admitted custody identity.

### 10.2 User slots and System locators

- User slots are owner-local.
- `0 < MaxOwnerSlots <= 255`.
- The reference profile exposes `0..=254`; `u8::MAX` is invalid.
- Default User creation selects the lowest free slot.
- Exact-slot creation requires that exact free slot.
- User close releases the slot.
- System creation allocates a unique locator.
- System close marks its locator vacant.
- Locator reuse creates a new Actor id but the same sovereign account.
- Actor ids and queue tickets never repeat.

Sovereign custody at an unindexed derived account does not block exact reattachment. Reserved-account and live-collision errors remain distinct (§12.4). A previously registered vacant System locator remains reattachable even if later host policy classifies its derived account as reserved; the exception applies only to that exact locator. Custody identity survives host account-provider removal, although host dust/reaping policy may remove value independently.

### 10.3 Custody reattachment and recovery

Process lifetime and custody lifetime are independent.

After Close (§9.3):

- The sovereign account remains;
- Its balances and adapter-exposed positions remain host-ledger state;
- No Actors authority exists until reattachment.

A fresh User Actor reattaches the same custody iff owner and exact slot are equal to the closed Actor. A fresh System Actor reattaches the same custody iff it reuses the same vacant System locator.

Reattachment inherits custody only. It does not inherit Contract, mutability, nonce, Trigger state, lifecycle, funding accumulator, run state, or guarantees.

Recovery uses ordinary authored Contracts and Tasks (§6.6). Clients MAY automate exact-slot recreation, funding, recovery Contract generation, withdrawal, and re-close. Actors provides no direct recovery-transfer call.

### 10.4 Protected minimum

For User Actors and the fee-native asset:

```text
protected_minimum = MinUserBalance
```

For other assets/classes:

```text
protected_minimum = host asset minimum balance
```

Current Action-fee reservation is also unavailable to Task debit (§7.5).

```text
spendable_balance = balance - current_action_fee_reservation
preservable_balance = spendable_balance - protected_minimum
```

All subtraction is saturating only where explicitly shown. Every authored debit, including `AllAvailable`, uses preservable capacity. No lifecycle branch grants source-exhaustion privilege.

---

## 11. Host effects and adapters

### 11.1 Canonical Task-effect ownership

Each effectful Actor Task invokes the same canonical host economic mechanism as its external equivalent.

Equivalence means identical:

- State-transition owner;
- Atomicity boundary;
- Synchronous consequences;
- Typed failure classification;
- Production effect Weight.

Actors MAY use narrow typed ports and need not construct a `RuntimeCall`. A cheaper shadow mechanism is forbidden.

Actors owns orchestration control only. The host mechanism owns Task-effect Weight and task-native fees.

### 11.2 Adapter interfaces

```rust
enum RetryClass { Temporary, Permanent }

struct TaskFailure {
  error: DispatchError,
  retry: RetryClass,
}

struct ExecutionContext<'a, A> {
  actor: &'a A,
  actor_type: ActorType,
}

trait AssetOps<A, I, B> {
  fn transfer(from: &A, to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn burn(who: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn mint(to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn balance(who: &A, asset: I) -> B;
  fn minimum_balance(asset: I) -> B;
  fn preflight_transfer(from: &A, to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
}

trait DexOps<A, I, B> {
  fn swap_exact_in(context: ExecutionContext<'_, A>, asset_in: I, asset_out: I, amount_in: B, tolerance: Perbill) -> Result<B, TaskFailure>;
  fn swap_exact_out(context: ExecutionContext<'_, A>, asset_in: I, asset_out: I, amount_out: B, authored_input_cap: B, tolerance: Perbill) -> Result<B, TaskFailure>;
}

trait LiquidityOps<A, I, B> {
  fn lp_assets(lp: I) -> Option<(I, I)>;
  fn add_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B, min_lp: B) -> Result<(B, B, B), TaskFailure>;
  fn remove_liquidity(who: &A, lp: I, a: I, b: I, amount: B, min_a: B, min_b: B) -> Result<(B, B), TaskFailure>;
  fn donate_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B, max_ratio_error: Perbill) -> Result<(B, B), TaskFailure>;
}

trait StakingOps<A, I, B> {
  fn stake(who: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn unstake(who: &A, asset: I, shares: B) -> Result<(), TaskFailure>;
  fn share_balance(who: &A, asset: I) -> B;
  fn share_asset(asset: I) -> Option<I>;
}

trait FeeCollector<A, I, B> {
  fn collect_fee(payer: &A, sink: &A, fee_asset: I, amount: B) -> DispatchResult;
}
```

Missing mutation capability fails closed.

### 11.3 Failure classification

Retryability MUST NOT derive from strings, module indexes, or broad token errors.

Temporary includes dynamic slippage, current quote/cap insufficiency, stale reference, liquidity movement, recipient deposit unavailability, and recoverable placement capacity.

Permanent includes malformed configuration, invalid provenance, missing static capability, monotonic namespace exhaustion, topology corruption, and invariant failure.

Unknown downstream error is Permanent.

`FundingUnavailable` is amount/action admission outcome, not `TaskFailure` (§6.3, §7.5).

### 11.4 Special rules

#### Swap

`DexOps` obtains a current executable quote internally, applies authored cap/tolerance, invokes the canonical Router, validates actual amounts, and returns typed failure. Actors exposes no independent quote surface.

For `SwapOut`:

```text
capacity_cap = preservable input balance

authored_cap =
  capacity_cap                         for LiveQuote
  min(capacity_cap, absolute_cap)      for Absolute

effective_max_in = min(authored_cap, quote + ceil(quote * tolerance))
```

Zero or insufficient cap is `FundingUnavailable` or Temporary according to whether dispatch was invoked (§6.3, §11.3). `SwapIn` requires positive actual output and output no worse than the authored tolerance relative to its current executable quote; 100% tolerance never permits zero output.

#### System swap guard

User swaps have no Actors-specific reference guard. System swaps require fresh nonzero directed reference values and MUST satisfy, before mutation and against returned actual amounts when they differ from quote:

```text
exec_out * ref_in * Perbill::ACCURACY
  >=
(Perbill::ACCURACY - max_deviation) * ref_out * exec_in
```

All products use widened checked arithmetic. Missing/stale/zero reference or excessive negative deviation is Temporary. The Router does not own or reinterpret this guard.

#### Liquidity

Ordered LP identity is validated at admission and execution. Add/Donate actual debits MUST remain within supplied caps. RemoveLiquidity MUST debit the exact resolved LP amount.

For `DonateLiquidity`, Actors derives `max_b = preservable_balance(asset_b)`. Zero `max_b` is `FundingUnavailable`; the adapter MUST NOT invent a larger cap.

#### Staking

Staking-share identity is admitted and stable. `Unstake(AllAvailable)` resolves to the full current share balance. Transferable staking receipts/NFTs, when provided by the host, remain ordinary custody (§10.3).

#### Certified AddressEvent ingress

Only generated certified producers create AddressEvent semantics (§4.7). Permitted protocol shapes are:

| Protocol | Ordering and atomicity owner |
| --- | --- |
| `PostMovementNotify` | read-only preflight, movement, one notify; producer storage transaction |
| `BlockAtomicPostDispatch` | read-only preflight, successful dispatch, one notify; block/import state transaction |
| `XcmTransactionalPrecommit` | read-only preflight, Actors precommit, consume/deposit holding; asset-transactor storage transaction |

Every certified protocol defines:

- Source/provenance;
- Read-only preflight;
- Atomicity owner;
- Actors consequence owner;
- Failure mapping;
- Complete Weight.

Uncertified movement is balance-only. Absent or Dormant destination is balance-only. Zero and self/no-op movement create no Actors ingress. Terminal substitution occurs before funding/readiness consequence (§9.2). Fee collection is never certified AddressEvent ingress.

#### Observation publication

Only generated certified publishers create observation revisions. Publication is `O(1)` and records the exact previous/current transition and monotonic revision. Deferred matching belongs to detector workers (§8.4).

---

## 12. Calls, events, APIs, and errors

### 12.1 Calls and authorization

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
clear_crossing_worker_fault
clear_observation_fanout_worker_fault
clear_wakeup_worker_fault
actor_prepass
```

Authorization:

| Call | Origin |
| --- | --- |
| User creation | signed creator |
| System creation/locator reuse/active limit | `SystemOrigin` |
| breaker/fault repair | configured control origin |
| User control | signed owner, subject to mutability (§9.1) |
| System control | signed owner or `SystemOrigin`, subject to mutability (§9.1) |
| sweep | any signed origin |
| prepass | mandatory unsigned inherent origin |

Creation charges ordinary transaction fee, Actor Creation Fee, and state-hold delta (§7.2).

`manual_trigger` follows Manual semantics (§4.7, §7.3).

`close_actor` is signed Mutable User control and follows Close (§9.3).

### 12.2 Events and ordering

The ordered event ABI is normative:

```rust
enum Event<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId> {
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
  ActorFaultRecorded { fault_id: FaultId, kind: ActorFaultKind, first_recorded_block: BlockNumber, context: FaultContext },
  CrossingWorkerFaultCleared { feed: ObservationFeedId, revision: Option<ObservationRevision>, class: CrossingWorkerFaultClass },
  ObservationFanoutWorkerFaultCleared { feed: ObservationFeedId, revision: ObservationRevision, subscriber_page: Option<u32>, class: CrossingWorkerFaultClass },
  WakeupWorkerFaultCleared { key: WakeupKey<BlockNumber>, page: WakeupPageId, class: CrossingWorkerFaultClass },
  ManualTriggerSet { actor_id: ActorId },
  TriggerOccurrenceProcessed { actor_id: ActorId, trigger_family: TriggerFamily, fee: Balance },
  PipelineFeeCharged { actor_id: ActorId, fee: Balance },
  ActionFeeCharged { actor_id: ActorId, cycle_nonce: u64, step_index: u32, actual_effect_weight: Weight, fee: Balance },
  FundingAccumulated { actor_id: ActorId, asset: AssetId, added: Balance, accumulated: Balance },
  SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 },
  IdleStarvationDetected { consecutive_blocks: u32 },
  IdleStarvationRecovered { consecutive_blocks: u32 },
}
```

Ordering:

1. Useful Trigger fee and latch commit before `TriggerOccurrenceProcessed` (§4.2, §7.3).
2. Opening emits `PipelineFeeCharged`, then `CycleStarted` (§5.2, §7.4).
3. A Step event precedes any Cycle boundary event caused by that Step (§6.5).
4. Cancellation emits `CycleCancelled`, then `CycleSummary(Cancelled)` (§5.6).
5. A closing Cycle boundary emits `CycleSummary`, then `ActorClosed` (§9.3); pure Idle close emits only `ActorClosed`.
6. `ActionFeeCharged` is the final receipt of a committed Action-bearing Attempt (§7.5).
7. Fee-collection or enclosing transaction failure emits no rolled-back event.
8. Zero-Step Opening emits `CycleStarted`, then `CycleSummary(Completed)`, then optional `ActorClosed` (§5.5).
9. Redundant latched activity emits no Trigger event (§4.3).
10. Active creation emits `ActorCreated` only; Dormant-to-Active emits `ActorActivated`. Contract replacement or deactivation emits cancellation events first when a run exists, then `ContractUpdated` or `ActorDeactivated` (§5.6, §9.5).

Exact fields and discriminants come from metadata.

### 12.3 Runtime APIs

```rust
enum TriggerFamily { Manual, AddressEvent, ObservationChange, ObservationCrossing, AtTime, Cadenced }

enum RunPhase<BlockNumber> {
  WaitingNextBlock { not_before: BlockNumber },
  WaitingRetry { not_before: BlockNumber },
  WaitingCadenceTick { due_tick: u64 },
  WaitingSignal,
  Paused,
  Breaker,
  Ready,
}

enum ActorEligibility<BlockNumber> {
  NotRegistered,
  Dormant,
  Active { terminal_reason: Option<CloseReason>, phase: RunPhase<BlockNumber> },
}

struct TriggerFeeBreakdown<TriggerFamily, Balance> {
  trigger_family: TriggerFamily,
  trigger_fee: Balance,
}

struct PipelineFeeBreakdown<Balance> {
  pipeline_machine_fee: Balance,
}

struct ActionFeeBreakdown<Balance> {
  maximum_effect_fee: Balance,
  actual_effect_fee: Option<Balance>,
}

struct StepProgress<BlockNumber> {
  terminal_reason: Option<CloseReason>,
  phase: RunPhase<BlockNumber>,
  cycle_nonce: u64,
  cursor: Option<u32>,
  total_steps: u32,
  completed_steps: u32,
  next_eligibility: Option<BlockNumber>,
  suspension: Option<SuspensionReason>,
  last_step_outcome: Option<StepOutcome>,
}
```

The runtime MUST expose named structures for:

- Complete semantic Contract and identities;
- Actor progress and current phase;
- Current control/effect maxima;
- Prospective Trigger fee;
- Prospective Pipeline Machine fee;
- Maximum next Action fee and last actual Action fee;
- Actor state hold and body footprint;
- Block resource limits/counters;
- Current bounded faults;
- Crossing capacity.

Clients MUST NOT assemble semantic Contracts from raw heads, chunks, pages, or locators.

No API exposes remaining Pipeline Machine budget because none exists (§7.4).

Simulation MUST be rollback-only and simulate at most one current Step under an explicit synthetic Actor Control and Shared Economic budget (§6.1, §7.6). It MUST use the same classifier, precondition, amount, Task, fee, outcome, and policy owners (§8.1, §6.2-§6.6, §7.4-§7.5). Waiting state returns `NotReady`; insufficient synthetic resource returns a resource-deferral result; fee-collector failure returns its typed simulation error. Simulation persists no state or event.

### 12.4 Errors and projections

Core classification errors:

```rust
enum ActorClassificationError {
  ActorInvariant,
  RunInvariant,
  ComputationOverflow,
}
```

They project exactly to dispatch/runtime-API/simulation errors and MUST NOT become absence, dormancy, waiting, or scheduler exhaustion.

```rust
enum Error {
  ActorIdOverflow, ActorNotFound, ActiveActorCapacityExceeded, ActiveActorCountInvariant,
  ActorIdentityCapacityExceeded, ActorIdentityCountInvariant, ActorInvariant, ActorAlreadyActive,
  ActorDormant, ActiveActorLimitExceedsQueueCapacity, ActiveActorLimitTooHigh,
  ActiveActorLimitTooLow, ActiveActorLimitBelowCurrent, ActorPaused,
  ContractStepsExceedOnIdleBudget, ExecutionDelayTooLong, GlobalCircuitBreakerActive,
  ImmutableActor, InsufficientBalance, InsufficientFee, InvalidAmountResolution, InvalidPredicate,
  InvalidAutoCloseNonce, InvalidScheduleWindow, InvalidSplitTransfer, InvalidTriggerConfiguration,
  InvalidTradeBound, InvalidRetryAttemptLimit, InvalidObservationMaxAge, SelfTransferNotAllowed,
  MintNotAllowedForUserActor, NotGovernance, NotOwner, OwnerSlotCapacityExceeded,
  OwnerSlotOccupied, InvalidOwnerSlot, ActorIdOccupied, SystemSovereignCapacityExceeded,
  SystemSovereignUnknown, SystemSovereignOccupied, SystemSovereignInvariant,
  SovereignAccountCollision, ReservedSovereignAccount, TooManyContractSteps, SnapshotUnavailable,
  FundingAccumulatorOverflow, QueueTicketExhausted, SchedulerIndexExhausted,
  AutoCloseNonceHorizonExceeded, ControlMutationRateLimited, QueueCapacityUnavailable,
  RetryLaterNotAllowedForImmutableActor, ActorRunNotFound, ActorRunInvariant, ComputationOverflow,
  EmptyPrecondition, ManualSourceDisabled, RecipientDepositUnavailable,
  ObservationSubscriptionCapacityExceeded, ObservationSubscriptionInvariant,
  InvalidObservationRevision, DirtyObservationCapacityExceeded, DirtyObservationInvariant,
  ObservationUnavailable, ObservationUninitialized, CrossingIndexCapacityExceeded,
  CrossingUserCapacityExceeded, CrossingIndexInvariant, CrossingGenerationExhausted,
  CrossingTransitionCapacityExceeded, CrossingTransitionInvariant, CrossingWorkerFaultNotFound,
  ObservationFanoutWorkerFaultNotFound, WakeupWorkerFaultNotFound, SystemActorTopologyInvalid,
  AdmissionBoundOverflow, StateHoldUnavailable, StateHoldInvariant, StateHoldOverflow,
  PrepassDuplicateOrStale, ResourceProtocolFailed, PrepassContextIncomplete,
}
```

Required distinguishable failure domains, represented either by a dedicated variant or by the documented classification projection, include:

- Absence/dormancy/active-state mismatch;
- Authorization/mutability;
- Invalid Contract/Trigger/Predicate/amount/trade/retry/window;
- Capacity and bound overflow;
- Slot/locator/collision/reserved account;
- State hold and fee collection;
- Queue/wakeup/detector faults and monotonic exhaustion;
- Stale admission or body authority projects to `ActorInvariant`; stale run authority projects to `ActorRunInvariant`;
- Control rate limit and breaker;
- Prepass/cutoff/resource protocol failure.

Required Close reasons include:

```rust
enum CloseReason {
  OwnerInitiated,
  TriggerAdmissionInsufficient,
  CycleAdmissionInsufficient,
  WindowExpired,
  CycleNonceExhausted,
  RetryAttemptsExhausted,
  ConsecutiveFailures,
  ProductiveCycleCompleted,
  AutoCloseNonceReached,
  SchedulerIndexExhausted,
}
```

Resolution outcomes are not pallet errors. `TaskFailure.error` MAY carry a stable dispatch diagnostic without converting the Attempt into a rejected extrinsic.

---

## 13. Storage, upgrades, configuration, and conformance

### 13.1 Storage and integrity

Generated descriptors own exact keys, hashers, prefixes, values, and page geometry.

Normatively required:

- Only canonical partitions from §3.5 and `ActorRunState` from §5.1;
- Bounded collections and exact reverse ownership;
- One live ticket and bounded wakeup authority per §8.2;
- No unbounded execution history;
- No persistent execution cache duplicating authored fields, Steps, cursor, snapshots, outcomes, lifecycle, or latch;
- No remaining Pipeline budget, fee cache, or generic cache-revalidation workset;
- Exact hold/state reconciliation (§7.2);
- `try_state` verification of Contract/body/admission, run, latch, topology, counters, funding, slots, locators, and orphans.

Orphan physical records are never semantic authority and are integrity failures unless they are transaction-local writes that roll back.

### 13.2 Runtime upgrades

Before first production genesis, a fresh canonical baseline MAY replace storage or ABI without migration compatibility.

After deployed production lineage, every storage or semantic rewrite MUST define:

- Source and target schemas;
- Bounded migration unit and progress owner;
- Execution gate;
- Weight;
- Interruption/resume/idempotence;
- Actor and custody disposition;
- Open-run `Cancel | PreserveWithProof` policy;
- Admission re-certification;
- Storage-version transition.

A Weight or fee-policy change may alter runtime admission identity but MUST NOT alter semantic Contract identity (§3.2, §3.4). Open paid Pipelines retain their paid service identity through the Cycle boundary unless the migration specification proves another disposition.

A deployed change MUST NOT make previously reattachable custody unreachable without explicit custody disposition (§10.3).

### 13.3 Runtime configuration

Required relations:

1. `0 < MaxContractSteps <= 255`; each host runtime selects its bounded value.
2. `0 < MaxOwnerSlots <= 255`; reference is 255.
3. `MaxRetryAttempts >= 2`; reference is 10.
4. `MaxContractSteps * MaxRetryAttempts` fits outcome counters.
5. Opening snapshot/result bounds cover every admitted Contract.
6. `MaxCrossingMembersPerFeed` covers at least 10,000 User memberships plus every separately bounded host-owned membership reserved for the feed.
7. Every queue, wakeup, detector, cohort, sweep, and worker bound is nonzero and owns one complete worst-case unit.
8. One maximum current-Step control/effect transition fits the guaranteed base pass (§7.6, §8.3).
9. Maximum admitted create, activate, update, deactivate, cancel, and close paths remain dispatchable under their owning call limits.
10. `MinUserBalance >= host minimum balance`.
11. `ActorCreationFee > 0`.
12. `WeightToFee` maps every nonzero User Trigger, Pipeline, and Action Weight upper bound to positive fee.
13. Trigger and Pipeline fee owners are disjoint (§7.3, §7.4).
14. Actor Control is floor one third of schedulable Weight; Shared Economic owns the remainder and its two base turns split floor/remainder (§7.6).
15. `MaxTemporalDelayTicks` and `MaxExecutionDelayBlocks` are clock-specific and representable.
16. `MaxSplitTransferLegs >= 2`.

Reference profile:

```text
TargetBlockTime = 6 seconds
CadenceTick = 500 milliseconds
MaxActiveActors = 10_000
MaxOwnerSlots = 255
MaxContractSteps = 12
MaxRetryAttempts = 10
MaxConsecutiveFailures = 10
MaxFundingTrackedAssets = 40
MaxOpeningSnapshotEntries = 24
MaxOpeningPredicateResults = 48
MaxPreconditionClauses = 4
MaxPredicatesPerClause = 4
MaxPredicatesPerStep = 4
MaxWhitelistSize = 16
MaxSplitTransferLegs = 8
MaxQueueLength = 10_000
MaxSweepBatch = 5
MinUserBalance = 5 * existential deposit
ActorCreationFee = 2 * existential deposit
MaxTemporalDelayTicks = 631_152_000
MaxExecutionDelayBlocks = 52_596_000
ActorControlLimit = floor(SchedulableBlockWeight / 3)
SharedEconomicLimit = SchedulableBlockWeight - ActorControlLimit
ActorBaseTurn = floor(SharedEconomicLimit / 2)
UserBaseTurn = SharedEconomicLimit - ActorBaseTurn
```

Measured runtime descriptors own page sizes, per-block worker counts, exact Weight limits, adapter maxima, and generated fee-envelope identities.

### 13.4 Conformance

A runtime conforms iff:

1. Metadata exposes only canonical public shapes;
2. Every semantic function follows its sole owner (§1.2);
3. Every cross-reference uses, rather than redefines, the owned function;
4. Every reachable transition is bounded, pre-admitted, and transactionally atomic at its defined boundary;
5. Trigger occurrence, latch, re-arm, and underfunding follow §4;
6. Opening, snapshots, nonce, zero-Step, and Cycle boundaries follow §5;
7. Q1 Step execution, error policies, and Tasks follow §6;
8. Creation, Trigger, Pipeline, Action, state-hold, and resource ownership follow §7 without overlap;
9. Classification, cutoff, FIFO, cohorts, faults, and liveness follow §8;
10. Mutability, terminal precedence, close, apoptosis, and breaker follow §9;
11. Sovereign custody survives process deletion and exact reattachment follows §10;
12. Each Task effect maps to one canonical host owner and typed failure surface (§11);
13. Calls, events, APIs, and errors follow §12;
14. Storage integrity and deployed upgrades follow §13.1-§13.2;
15. Generated Weight upper-bounds every admitted control/effect branch in both dimensions;
16. Simulation is rollback-only and observationally equivalent to zero or one production current-Step transition.

---

_End of specification._
