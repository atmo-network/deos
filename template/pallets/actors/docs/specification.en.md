# DEOS Actors Specification

- **Scope**: Bounded economic actor runtime contract
- **Target**: `pre-1.0.0`
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase. This document defines runtime behavior and semantic type meaning. Runtime metadata defines exact SCALE encoding. Generated storage and Weight descriptors define physical layout and measured values.

Rust code blocks that do not declare public types, interfaces, events, errors, or calls are normative semantic pseudocode. They define branch order, state dependencies, and error propagation, but not source-level identifiers, ownership syntax, trait bounds, or metadata encoding.

---

## 1. Core Contract

- Equal state and block context MUST produce equal behavior.
- Work MUST be O(1) or O(K) under explicit finite bounds and admit complete `Weight(RefTime, ProofSize)` before mutation.
- An Actor Contract contains bounded linear `ContractSteps`: no loops, jumps, nested contracts, opaque dispatch, Task-authored memory, or authored whole-contract rollback.
- Every admitted path ends in a named completion, skip, failure, suspension, cancellation, close, or state-preserving deferral; unknown capability or failure fails closed.
- Actor Contract identity is the domain-separated digest of canonical typed SCALE bytes plus explicit runtime binding; JSON text, whitespace, object-key order, comments, labels, and client serialization choices MUST NOT affect it.
- `ActorIdentity`, `ActorHot`, `ActorContract`, `ActorFunding`, and optional `ContinuationState` are the only canonical actor partitions; composite views are read-only.
- User and System share one strict FIFO, temporal-readiness layer, cutoff, and service order.
- One generated runtime `WeightInfo` owns every numeric Weight and every Weight-derived User fee.
- Attempt Weight and User fee bounds are derived from the current bounded contract at use time; no derived Weight or fee state is stored.
- Arithmetic is checked unless a formula explicitly names saturating arithmetic. A positive resolved exact amount MUST NOT be silently reduced to fit capacity; explicit cap formulas MAY use `min` or saturating subtraction, and insufficient exact capacity yields the specified non-executable outcome.
- Close deletes actor semantics, preserves sovereign balances at the deterministic account, releases the User slot or System locator, and performs no transfer.
- An authored terminal custody drain MAY transfer one explicitly named asset immediately before `ProductiveCycleCompleted` close. Custody left by any other close requires fresh reattachment to the same deterministic account.
- User creation pays only on committed creation; User executable steps are attempt-priced; System execution is Actors-fee-exempt.
- Actors has no recurring rent, task-fee refund, implicit balance discovery, or background solvency scan.

### 1.1 Terms

| Term | Contract |
| --- | --- |
| Rejected control transition | The Actors call returns `Err`; Actors state/events, subscriptions, scheduler state, and creation-fee movement equal pre-state. Host fee/nonce accounting is excluded. |
| Provisional task commit | A task layer succeeds inside an admitted scheduler attempt but remains reversible until the enclosing attempt commits. |
| Committed unsuccessful attempt | An admitted attempt durably commits failure or suspension; collected Section 4.3 fees remain charged. It is not a rejected transition. |
| Rolled-back scheduler attempt | The enclosing attempt fails before durability; queue consumption, nonce/attempt, snapshots, fees, tasks, events, and scheduler state equal pre-attempt state. |
| Opening | The atomic transition that starts one fresh logical cycle, captures its immutable bases, increments `cycle_nonce`, consumes the current latch and funding accumulator, and emits `CycleStarted`. |
| Attempt | One admitted execution of a fresh cycle or Continuation suffix. Retries are new attempts in the same cycle. |
| Committed effectful task | A successful `Transfer`, `SplitTransfer`, `SwapIn`, `SwapOut`, `AddLiquidity`, `RemoveLiquidity`, `Burn`, `Mint`, `Stake`, `DonateLiquidity`, or `Unstake` task layer that becomes durable with its enclosing attempt. `StopCycle`, skipped resolution, funding suspension, task failure, and rolled-back task success do not count. |
| Cycle | One logical execution from opening through completion, terminal failure, suspension chain, cancellation, or close. |
| Placement | One live FIFO ticket, one exact temporal target for the earliest pending ordinary or terminal requirement, or none. |
| Membership | A physical scheduler/subscription record authorized by actor-local or reverse-index ownership. Physical records without current ownership are stale and have no authority. |
| Semantic replacement | A submitted canonical value differs from the stored canonical value. Canonical equality is an exact no-op before rate limiting, clock mutation, cancellation, placement reconstruction, storage writes, or events. |
| Economic apoptosis | Mandatory User close with `BalanceExhausted` or `FeeBudgetExhausted`. |
| Custody reattachment | Fresh actor creation with the same User owner and exact slot, or the same vacant System locator, deriving the same sovereign account and inheriting only its current adapter-exposed custody. |
| Terminal custody drain | A sole-step `Transfer(AllAvailable)` plan under `CloseAfterProductiveCycle` that may consume the protected minimum because successful transfer and close commit atomically. |

A step failure inside an admitted attempt is not a rejected transition. Authored whole-plan rollback is forbidden; rollback of the enclosing scheduler transaction remains required on its own failure.

### 1.2 Non-goals

Actors defines no ownership transfer, arbitrary dispatch, loops, same-block actor-graph continuity, priority lane, actual-Weight refund, readiness-bypassing simulation, signal-payload amount resolution, adaptive rent/deposit pricing, generic migration engine, close-time asset enumeration, implicit multi-asset refund, or Router route semantics. Uncertified balance movements are not retroactively interpreted as Actors ingress.

## 2. Actor Model

### 2.1 Canonical State

```rust
type ActorId = u64;
type OwnerSlot = u8;
type OwnerSlotBitmap = [u8; 32];
type SystemSovereignId = u64;
type QueueTicket = u64;
type WakeupPageId = u64;
type WakeupSlot = u32;
type ObservationRevision = u64;

enum ActorType { User, System }
enum InitialLifecycle { Dormant, Active }
enum ActorClass { User { owner_slot: OwnerSlot }, System { sovereign_id: SystemSovereignId } }
enum SystemSovereignState { Vacant, Occupied(ActorId) }
enum Mutability { Mutable, Immutable }
enum ActiveLifecycle { Active, Paused }
enum CycleState { Idle, Suspended }

struct ActorIdentity<AccountId, BlockNumber> {
  sovereign_account: AccountId, owner: AccountId, actor_class: ActorClass,
  mutability: Mutability, cycle_nonce: u64, last_control_mutation_block: BlockNumber,
}
struct WakeupPointer<BlockNumber> {
  block: BlockNumber, page_id: WakeupPageId, slot: WakeupSlot,
}
struct ActorHot<BlockNumber> {
  lifecycle: ActiveLifecycle, cycle_state: CycleState,
  unsuccessful_attempt_streak: u32,
  pending_signal: bool, queue_ticket: Option<QueueTicket>,
  wakeup_pointer: Option<WakeupPointer<BlockNumber>>, terminal_at: Option<BlockNumber>,
  schedule_anchor: BlockNumber, last_cycle_block: Option<BlockNumber>,
}
struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy> {
  trigger: Trigger, cooldown_blocks: u32,
  window: Option<ScheduleWindow<BlockNumber>>, steps: Steps,
  funding: FundingPolicy, completion: CompletionPolicy,
  auto_close_at_cycle_nonce: Option<u64>,
}
struct ActorFunding<AssetId, Balance> {
  funding_accumulated: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
}
struct ContinuationState<BlockNumber, AssetId, Balance> {
  cursor: u32, unsuccessful_attempts_at_cursor: u32,
  last_attempt_block: BlockNumber,
  opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxOpeningSnapshotEntries>,
  opening_predicate_results: BoundedVec<Result<bool, PredicateError>, MaxOpeningPredicateResults>,
  funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  cumulative_outcomes: OutcomeTotals,
}
struct OutcomeTotals {
  executed_steps: u32, committed_effectful_tasks: u32,
  precondition_skips: u32, skipped_resolution: u32,
  skipped_funding_unavailable: u32, failed_steps: u32,
}

enum CloseReason {
  OwnerInitiated, BalanceExhausted, FeeBudgetExhausted, WindowExpired,
  CycleNonceExhausted, RetryAttemptsExhausted, ConsecutiveFailures,
  ProductiveCycleCompleted, AutoCloseNonceReached, SchedulerIndexExhausted,
}
enum CycleResult { Completed, Failed, Cancelled }
enum SuspensionReason { FundingUnavailable, Temporary }
enum CancellationReason {
  Explicit, ContractReplaced, Deactivated, Closing(CloseReason),
}
enum StepSkippedReason { PreconditionFalse, ResolutionSkipped, FundingUnavailable }
```

Relations:

- Active iff identity, hot, contract, and funding partitions exist; Dormant iff only identity exists.
- `ContinuationState` exists iff `cycle_state == Suspended`.
- `ActorClass` solely determines class; `ActorType` is derived and never stored.
- A composite actor value is read-only and MUST NOT become another write model.
- One runtime `MaxContractSteps` applies to both classes.
- `funding_tracked_assets` is the exact bounded derivation of the current Contract Steps; no duplicate count is stored.
- `terminal_at` is `end + 1` exactly when a schedule window exists and otherwise `None`.
- Dormant creation performs no contract scan, subscription, funding tracking, readiness, or placement.
- Active admission derives the complete current cycle or suffix Weight and User fee envelope before collection or mutation.
- `WakeupPointer` authorizes the actor's single physical temporal membership for the earliest pending ordinary or terminal requirement. `terminal_at` independently authorizes terminal classification. Runtime metadata/storage descriptors define physical fields and topology.

### 2.2 Class and Mutability

User creation is signed, makes the caller the owner, consumes one owner slot, and pays User Actors fees. System creation requires `SystemOrigin`, names an explicit owner, consumes no User slot, and pays no User Actors fee. Both classes share FIFO order.

Actor-scoped authorization is:

| Actor | Authorized control origin |
| --- | --- |
| User | signed owner |
| System | signed owner or `SystemOrigin` |

This rule includes `manual_trigger`. Unauthorized signed control returns `NotOwner`; an origin accepted by `SystemOrigin` that targets a User actor returns `NotGovernance`.

Mutable control may replace schedule/window, Contract Steps/completion policy, funding policy, lifecycle, Continuation, close target, or Active/Dormant state. Dormant creation requires `Mutable`; Immutable creation MUST install an Active Actor Contract because no later activation authority exists.

Immutability covers the Actor Contract and control authority only. It does not freeze runtime Weight, fee conversion, minimum-balance policy, adapter behavior, or host economic parameters.

User Immutable:

- Rejects every semantic replacement, pause/resume, activate/deactivate, cancellation, auto-close change, and explicit close;
- MAY invoke an authored Manual readiness signal;
- Cannot use `RetryLater`;
- Remains bound to its authored role while economically viable;
- MUST close through `BalanceExhausted` or `FeeBudgetExhausted` when scheduler classification or permissionless sweep observes that it cannot fund the current full-plan or Continuation-suffix attempt.

Economic apoptosis applies to Mutable and Immutable User actors. Only scheduler classification and permissionless sweep evaluate its predicates; Actors performs no background solvency scan.

After any User close, including economic apoptosis of User Immutable, the former owner MAY create a fresh Mutable User actor at the released exact slot. The new actor inherits no mutability, contract, authority, lifecycle, or terminal state from the closed actor; Section 2.3 custody reattachment is the only continuity.

System Immutable:

- Rejects every actor-scoped control;
- Cannot contain Manual or `RetryLater`;
- Pays no Actors attempt fee;
- Is not required to contain a finite actor-lifetime terminal path; every admitted attempt remains bounded and reaches a named result;
- Remains Active until a mandatory terminal predicate or Section 9.4 upgrade removes it.

`2 <= RetryLater.max_attempts <= MaxRetryAttempts`; `max_attempts` counts unsuccessful executions of the cursor including the opening attempt, so it permits at most `max_attempts - 1` retries.

`SystemOrigin` has no call-level override for System Immutable existence or contract state. A runtime upgrade MAY override it only under Section 9.4 and MUST define actor disposition, custody consequences, and Continuation policy.

### 2.3 Sovereign Accounts and Identifiers

```text
User seed   = Blake2_256(SCALE(ActorsPalletId, b"user", owner, owner_slot))
System seed = Blake2_256(SCALE(ActorsPalletId, b"system", system_sovereign_id))
account     = HostSovereignAccountDeriver(seed)
```

The decoder MUST be total and deterministic for every 32-byte seed.

- `OwnerSlotBitmap` is owner-local. It has 256 bits; valid slots satisfy `owner_slot < MaxOwnerSlots`, and every bit outside that range is zero.
- The reference profile exposes slots `0..=254`; bit `255` is a permanently invalid sentinel so `u8::MAX` never denotes an owned slot.
- Default User creation selects the lowest free valid slot; exact-slot creation requires a valid free slot; close clears it.
- `create_system_actor` allocates a fresh locator with `system_sovereign_id = actor_id`, records `Occupied(actor_id)`, and consumes lifetime locator capacity.
- `create_system_actor_at_sovereign_id` requires an existing `Vacant` locator, assigns a fresh `actor_id`, and records `Occupied(fresh_actor_id)`; unknown and occupied locator cases are distinct errors.
- Close marks a System locator `Vacant`; deactivation leaves it occupied; reuse never reuses an actor id.
- `SystemSovereignCount` includes vacant locators; reuse does not change it.
- The host deriver MUST map the tagged seed deterministically without a runtime panic, preserve owner/slot and System-locator separation, and keep every previously admitted custody identity stable.
- `SovereignIndex` covers current Active or Dormant ownership only. Nonzero balance or host provider state at an otherwise unindexed derived account is custody, not a live Actors collision, and MUST NOT block creation or reattachment.
- Fresh User-slot and fresh System-locator derivation reject reserved accounts. Exact User-slot reattachment remains a fresh derivation and applies the same reserved-account check.
- Reattachment to an already registered vacant System locator is permitted for that exact locator even if host policy later classifies its derived account as reserved. This exception applies only to that registered locator.
- A deployed runtime change MUST NOT make previously reattachable custody unreachable without the Section 9.4 custody disposition. Live collision, reserved collision, unknown/occupied locator, and capacity exhaustion remain distinct errors.
- `NextActorId` and queue tickets checked-increment and never repeat.
- A fresh User actor derives the closed actor's sovereign account iff both signed owner and exact `owner_slot` equal those used for the closed actor. The same numeric slot under another owner derives a different account.
- A fresh System actor derives the closed actor's sovereign account iff it reuses the same vacant `system_sovereign_id`.
- Such reattachment inherits adapter-exposed custody only. Owner/class binding, mutability, contract, nonce, funding, Continuation, clocks, readiness, and guarantees are newly created.
- Custody derivation survives host account-provider removal.
- Every Actors-declared ordinary debit other than a Section 3.4 terminal custody drain obeys the protected minimum in Section 3.3.
- A successful ordinary User fee-native debit preserves `MinUserBalance`; a terminal custody drain may consume it only after reserving and collecting the current attempt fee. Adapters MUST NOT add hidden debits.
- External host actions MAY reap or dust an account; deterministic reattachment does not recreate value removed by host policy.
- After close, no Actors authority exists over preserved balances until successful reattachment; knowledge of the deterministic account alone grants no control.

### 2.4 Lifecycle, Economic Viability, and Terminal Order

Terminal predicates are evaluated in this precedence when their owning transition applies. The first satisfied predicate is selected and lower-precedence predicates are not evaluated:

| State | Close reason |
| --- | --- |
| `current_block > window.end` | `WindowExpired` |
| User fee-native balance `< MinUserBalance` | `BalanceExhausted` |
| User available fee budget `< attempt_fee_envelope(plan, cursor, User).total` | `FeeBudgetExhausted` |
| `cycle_nonce == u64::MAX` before activation or fresh opening | `CycleNonceExhausted` |
| cursor-local unsuccessful executions reach `max_attempts` | `RetryAttemptsExhausted` |
| `unsuccessful_attempt_streak` reaches `MaxConsecutiveFailures` | `ConsecutiveFailures` |
| completed productive cycle under `CloseAfterProductiveCycle` | `ProductiveCycleCompleted` |
| non-suspended cycle reaches auto-close target | `AutoCloseNonceReached` |
| due materialization or post-attempt placement exhausts a monotonic scheduler index | `SchedulerIndexExhausted` |

For User economic viability:

```text
cursor =
  ContinuationState.cursor when Suspended;
  0 otherwise

fee_native_balance =
  AssetOps::balance(sovereign_account, FeeNativeAssetId)

available_fee_budget =
  fee_native_balance.saturating_sub(MinUserBalance)

required_attempt_fee =
  attempt_fee_envelope(contract_steps, cursor, User).total
```

System ignores User balance and fee predicates. User resource predicates apply regardless of mutability. Productive close precedes auto-close. Auto-close applies only when `cycle_state == Idle`. Windows are inclusive; terminal readiness occurs at representable `end + 1`. Attempt finalization evaluates `ProductiveCycleCompleted` only after `Completed`, then evaluates `AutoCloseNonceReached` after any non-suspended terminal cycle.

Terminal ownership:

- `WindowExpired` is checked by actor-targeting control, address-event notification, scheduler classification, and sweep.
- `BalanceExhausted` and `FeeBudgetExhausted` are checked by scheduler classification and sweep.
- `CycleNonceExhausted` is checked before Dormant activation, fresh-cycle opening, scheduler classification, or sweep. Activation substitutes close of the Dormant identity.
- Attempt finalization applies retry-, failure-, productive-, post-cycle auto-close, and permanent scheduler-index reasons. Due materialization also closes on permanent queue-index exhaustion before any attempt. Scheduler classification and sweep close an actor when its stored retry count, failure count, Idle cycle nonce, or Idle auto-close target already satisfies a terminal predicate; neither infers `ProductiveCycleCompleted` or `SchedulerIndexExhausted` from stored state. `AutoCloseNonceReached` is not due while Suspended.
- Under the global breaker, due temporal membership MAY materialize, but scheduler-owned automatic close does not execute. Explicit Mutable close, actor-targeting expiry substitution, ingress-triggered expiry close, and permissionless sweep remain available.

Address-event notification performs inline terminal close only for `WindowExpired`. User resource predicates are evaluated after any credited value only when scheduler classification or sweep later runs.

An admitted attempt is unsuccessful exactly when it commits `Suspended` or terminal `Failed`.

- A `RetryLater` suspension candidate increments `unsuccessful_attempts_at_cursor` and `unsuccessful_attempt_streak` exactly once before terminal-bound evaluation; it commits `CycleSuspended` only when neither bound is reached.
- A non-retry terminal `Failed` attempt increments `unsuccessful_attempt_streak` exactly once.
- `Completed` resets `unsuccessful_attempt_streak` to zero before rearm and completion-driven close evaluation.
- Deferral, cancellation, pause/resume, exact no-op, an advancing `FundingUnavailable` skip, and an individual `ContinueNextStep` failure do not independently change either counter.
- Fresh Active installation and semantic Contract Steps or completion-policy replacement reset the global streak.
- Schedule and funding-policy replacement preserve the global streak.

For a suspension candidate at `cursor`, `prior_continuation` is the immutable `ContinuationState` loaded at attempt start, or `None` for a fresh opening.

```rust
let next_local = match prior_continuation {
  Some(previous) if previous.cursor == cursor =>
    checked_add(previous.unsuccessful_attempts_at_cursor, 1)?,
  _ => 1,
};

let next_global =
  checked_add(unsuccessful_attempt_streak, 1)?;

fn backoff(index: u32) -> u32 {
  1u32.checked_shl(index).unwrap_or(8).min(8)
}

if next_local >= max_attempts {
  finalize_failed_and_close(CloseReason::RetryAttemptsExhausted)?;
} else if next_global >= MaxConsecutiveFailures {
  finalize_failed_and_close(CloseReason::ConsecutiveFailures)?;
} else {
  persist_suspension(cursor, next_local, next_global)?;
}
```

`backoff(checked_sub(next_local, 1)?)` determines retry timing under Section 3.2 and is not stored separately. Local precedence wins when both bounds are reached by the same attempt. `ContinuationState.cursor` is the previous suspended cursor; no additional cursor field exists.

A fresh opening or first suspension at a cursor different from `prior_continuation.cursor` starts cursor-local counting at `1`. Repeated suspension at the same stored cursor checked-increments the stored count. `(actor_id, cycle_nonce, block_number, event_index)` identifies each attempt; no cycle-global attempt ordinal is stored or emitted.

A non-retry terminal `Failed` attempt applies only `next_global`; threshold close follows `CycleSummary(Failed)`.

Fresh Active creation and activation install one Active epoch:

| Surface | Installed value |
| --- | --- |
| identity nonce | `0` for fresh creation; preserved Dormant nonce for activation |
| lifecycle/cycle | `Active / Idle`; no Continuation |
| schedule clocks | `schedule_anchor = max(now, window.start)` when a future window exists, otherwise `now`; `last_cycle_block = None`; `terminal_at = end + 1` or `None` |
| readiness | `pending_signal = false`; one canonical FIFO/temporal path derived from the schedule, or none when no readiness is due |
| failure/economics | `unsuccessful_attempt_streak = 0`; empty accumulator; tracked assets rederived from the contract |
| subscriptions | exact feeds derived from trigger sources |
| persistent control clock | identity `last_control_mutation_block = now` |

Activation does not increment `cycle_nonce`. Every later fresh cycle computes:

```text
next_cycle_nonce = checked_add(stored_cycle_nonce, 1)
```

and `CycleStarted.cycle_nonce == next_cycle_nonce`. Retries reuse the opened nonce. A Dormant identity carrying nonce `n` remains at `n` on activation and opens its next cycle at `n + 1`.

Transition deltas:

| Transition | Preserved / changed |
| --- | --- |
| pause on Paused / resume on Active | exact no-op before rate check; no event |
| pause | preserve contract, funding, Continuation, failure streak, latch, clocks, and terminal target; remove ordinary readiness and retain only terminal placement when present |
| resume | preserve all semantic state; reconstruct the earliest ordinary or terminal placement from current state |
| semantic Actor Contract replacement with schedule/window change | cancel Continuation, diff subscriptions, reset `schedule_anchor` and `terminal_at`, apply every authored field atomically, recompute tracked assets when steps change, preserve the funding accumulator except entries no longer tracked, reset the failure streak when steps change, preserve the latch, then reconstruct placement |
| semantic Actor Contract replacement without schedule/window change | cancel Continuation, apply every authored field atomically, recompute tracked assets and prune untracked accumulator entries when steps change, reset the failure streak when steps change, preserve schedule clocks and latch, then reconstruct placement only if cancellation removed a retry path |
| auto-close target change | change only the target; do not cancel Continuation or consume the control-mutation limit |
| explicit `cancel_continuation` | require Suspended; emit `CycleCancelled(Explicit)`, then `CycleSummary(Cancelled)` from stored cumulative outcomes; delete `ContinuationState`; set `cycle_state = Idle`; preserve identity, nonce, contract, funding accumulator/tracked set, `pending_signal`, failure streak, schedule clocks, auto-close target, and balances; reconstruct the earliest ordinary or terminal placement; update the persistent control clock |
| deactivation | cancel Continuation and delete the complete Active epoch while preserving identity, locator, nonce, persistent control clock, and balances |

Deleting `ContinuationState` removes its cursor, attempt, unsuccessful counter, `last_attempt_block`, opening snapshot, funding snapshot, and stored cumulative outcomes after the cancellation events have projected those outcomes.

Every creation initializes `last_control_mutation_block` to the current block; genesis uses block zero. The clock is non-optional and survives deactivation.

`activate_actor`, `deactivate_actor`, semantic `pause_actor`, semantic `resume_actor`, semantic `update_contract`, and `cancel_continuation` are limited to one committed call per actor per block. Exact no-op returns before checking or updating the clock. `manual_trigger`, auto-close target calls, explicit close, sweep, global controls, ingress, and internal scheduler/cleanup transitions are exempt. Expiry-substituted close does not update a clock it immediately deletes. Rejection uses `ControlMutationRateLimited`.

Close performs bounded deletion/index repair, leaves every balance at the sovereign account, and releases the User slot or marks the System locator vacant. It never enumerates assets or selects a refund recipient. `Persistent` remains Active after completion; `CloseAfterProductiveCycle` requires `committed_effectful_tasks > 0`.

### 2.5 Continuation, Funding Delta, and Cycle Accounting

Explicit Continuation cancellation requires Mutable control and `cycle_state == Suspended`.

Semantic Actor Contract replacement; deactivation; close; expiry; or incompatible upgrade cancels before changed meaning applies. Exact no-op and auto-close-target changes do not cancel. Pause/resume and breaker preserve Continuation.

Cancellation performs no compensation, funding restoration, prefix rollback, or balance movement and emits `CycleCancelled`, then `CycleSummary(Cancelled)`. Before a retained Active actor is re-primed, cancellation exactly invalidates its current wakeup slot and reverse pointer in the same transaction; a latched next-run signal cannot create a new pointer behind an old physical slot.

If close encounters a Continuation, cancellation uses `Closing(reason)` and precedes `ActorClosed(reason)`. A pure close means no Continuation exists and emits only `ActorClosed`.

`update_contract(actor_id, contract)` replaces the complete authored Actor Contract atomically. Exact equality returns before rate limiting, clock mutation, writes, fees, and events. When trigger/cooldown/window, Steps, or funding changes, replacement cancels Continuation with `ContractReplaced`. Completion and auto-close-only replacement preserve a live Continuation. One successful semantic replacement emits only `ContractUpdated { actor_id }`.

Funding authority is independent from trigger matching:

| Policy | Authoritative funding acceptance |
| --- | --- |
| `OwnerOnly` | `provenance == Signed` and concrete `source == owner` |
| `SignedAllowlist` | `provenance == Signed` and concrete `source` is allowlisted |
| `RuntimePolicy` | `FundingAuthority::permits` accepts the exact source/provenance pair; the all-`None` pair MUST be denied |
| `AnyVerifiedIngress` | `source.is_some()` or `provenance.is_some()`; certification alone does not make the all-`None` pair acceptable |

Contract admission derives bounded `funding_tracked_assets`, including `StakingOps::share_asset(position)` for `PercentageOfLastFunding` used by Unstake.

`AnyVerifiedIngress` permits third parties to shape the next funding basis only by delivering real accepted value; it grants no withdrawal authority and every debit remains bounded by current capacity.

A credit in an untracked asset, or a tracked credit rejected by funding policy, is balance-only: it does not mutate `funding_accumulated` and emits no `FundingAccumulated`; trigger matching may still latch readiness. A tracked accepted positive credit checked-adds to the accumulator. Bound or arithmetic overflow returns `FundingAccumulatorOverflow` and rolls back the producer transaction.

When semantic plan/completion replacement removes an asset from the tracked set, deleting its accumulator entry changes only future `PercentageOfLastFunding` resolution. It does not move, burn, reserve, lock, or otherwise alter the corresponding balance.

`PercentageOfLastFunding` means accepted tracked funding since the previous logical cycle opening in the current Active epoch, or since Active installation for the first cycle.

On suspension, the persisted funding snapshot is the exact projection of the pre-clear funding snapshot onto assets referenced by `PercentageOfLastFunding` in `contract_steps[cursor..]`, including the unresolved cursor step and mapped Unstake share assets. Entries outside that suffix cannot be read because the cursor never decreases. Projection retains every present suffix-referenced entry and synthesizes no missing key. Absence continues to resolve as `FundingUnavailable`.

Later funding belongs to the next cycle. Funding accepted after opening cannot make a missing or zero current funding-snapshot entry available to a suspended cursor; `RetryLater` does not wait for post-opening funding. Completion, failure, cancellation, pause, and breaker neither restore the consumed snapshot nor alter the next accumulator. Deactivation and close delete accumulation.

A cycle completes at plan end or successful `StopCycle`; accepted `ContinueNextStep` failures may coexist with `Completed`. `AbortCycle`, Permanent `RetryLater`, exhausted bounds, and cancellation do not complete. `cycle_nonce` increments once per cycle; retries reuse it and increment `attempt`; `last_cycle_block` records opening and `last_attempt_block` the latest admitted attempt. Deferral changes no cycle state or event stream. Signals during an open cycle affect only possible future readiness.

## 3. Actor Contract Model

### 3.1 Public Types

```rust
struct ActorContract<Trigger, BlockNumber, Steps, FundingPolicy> {
  trigger: Trigger, cooldown_blocks: u32,
  window: Option<ScheduleWindow<BlockNumber>>, steps: Steps,
  funding: FundingPolicy, completion: CompletionPolicy,
  auto_close_at_cycle_nonce: Option<u64>,
}
struct ScheduleWindow<BlockNumber> { start: BlockNumber, end: BlockNumber }
enum Trigger<AccountId, AssetId, FeedId> {
  Manual,
  AddressEvent { source_filter: SourceFilter<AccountId>, asset_filter: AssetFilter<AssetId> },
  ObservationChange { feed: FeedId },
  ObservationCrossing { feed: FeedId, direction: CrossingDirection,
    threshold: u128, rearm_threshold: u128 },
  Cadenced { every_ticks: u64 },
}
enum CrossingDirection { Rising, Falling }
enum SourceFilter<AccountId> { Any, OwnerOnly, Whitelist(BoundedVec<AccountId, MaxWhitelistSize>) }
enum AssetFilter<AssetId> { Any, Whitelist(BoundedVec<AssetId, MaxWhitelistSize>) }
enum FundingSourcePolicy<AccountId> {
  OwnerOnly, SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>),
  RuntimePolicy, AnyVerifiedIngress,
}
enum FundingProvenance { Signed, InternalProtocol, Xcm }
struct AddressEvent<AccountId, AssetId, Balance> {
  destination: AccountId, source: Option<AccountId>, asset: AssetId,
  amount: Balance, provenance: Option<FundingProvenance>,
}

enum AmountResolution<Balance> {
  Fixed(Balance), PercentageOfCurrent(Perbill), PercentageAtOpening(Perbill),
  PercentageOfLastFunding(Perbill), AllAvailable,
}
enum OpeningSurface<AssetId> {
  PreservableAsset(AssetId), TargetAsset(AssetId), StakingShares(AssetId),
}
enum InputLimit<Balance> { LiveQuote, Absolute(Balance) }

struct Step<Predicate, Task> {
  precondition: Option<Precondition<Predicate, MaxPreconditionClauses, MaxPredicatesPerClause>>,
  task: Task, on_error: StepErrorPolicy,
}
enum ObservationTiming { Opening, Current }
struct TimedPredicate<P> { timing: ObservationTiming, predicate: P }
struct Precondition<P, MaxClauses, MaxPerClause> {
  clauses: BoundedVec<BoundedVec<TimedPredicate<P>, MaxPerClause>, MaxClauses>,
}
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

struct SplitLeg<AccountId> { to: AccountId, share: Perbill }
enum Task<AccountId, AssetId, Balance> {
  Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
  SplitTransfer { asset: AssetId, amount: AmountResolution<Balance>,
    legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs> },
  SwapIn { asset_in: AssetId, amount_in: AmountResolution<Balance>,
    asset_out: AssetId, slippage_tolerance: Perbill },
  SwapOut { asset_out: AssetId, amount_out: AmountResolution<Balance>,
    asset_in: AssetId, input_limit: InputLimit<Balance>, slippage_tolerance: Perbill },
  AddLiquidity { asset_a: AssetId, asset_b: AssetId,
    amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance>,
    min_lp_out: Balance },
  RemoveLiquidity { lp_asset: AssetId, asset_a: AssetId, asset_b: AssetId,
    lp_amount: AmountResolution<Balance>, min_amount_a: Balance, min_amount_b: Balance },
  Burn { asset: AssetId, amount: AmountResolution<Balance> },
  Mint { asset: AssetId, amount: AmountResolution<Balance> },
  Stake { asset: AssetId, amount: AmountResolution<Balance> },
  DonateLiquidity { asset_a: AssetId, asset_b: AssetId,
    max_amount_a: AmountResolution<Balance>, max_ratio_error: Perbill },
  Unstake { asset: AssetId, shares: AmountResolution<Balance> },
  StopCycle,
}
enum StepErrorPolicy { AbortCycle, ContinueNextStep, RetryLater { max_attempts: u32 } }
enum CompletionPolicy { Persistent, CloseAfterProductiveCycle }
```

Every Active Actor Contract contains `1..=MaxContractSteps`; empty `ContractSteps` returns `EmptyContractSteps`, and oversized `ContractSteps` returns `TooManyContractSteps`.

Every `Task` variant contains at most two `AmountResolution` fields. Each contributes at most one amount `OpeningSurface` exactly for `PercentageAtOpening`. Each distinct `Opening` Predicate contributes one frozen `Result<bool, PredicateError>`; `Current` Predicates contribute none. With `S = MaxContractSteps`, `A = 2`, and `P = MaxPredicatesPerStep`, configuration requires `MaxOpeningSnapshotEntries = S * A` and `MaxOpeningPredicateResults = S * P`; the reference aggregate opening bound is 48.

For each Step, `1 <= clauses <= MaxPreconditionClauses`, `1 <= predicates_per_clause <= MaxPredicatesPerClause`, and the sum of clause predicates is at most `P`. Opening capture plus full Step evaluation therefore costs at most `2 * P` atomic evaluation units per Step and `2 * S * P` per attempt. Weight and User evaluation fees split those units into chunks of at most `P` before calling the benchmarked `predicate_set_evaluation` component; no runtime call relies on its defensive clamp.

Each Actor Contract contains exactly one Trigger. `Manual`, `AddressEvent`, `ObservationChange`, and `ObservationCrossing` are externally signalled; `Cadenced` is internally timed and has no signal source. No Trigger variant contains an OR-set, nested Trigger, priority, or secondary admission source.

Every `Whitelist` and `SignedAllowlist` is nonempty, duplicate-free, and strictly ordered by canonical SCALE bytes. Runtime admission MUST reject non-canonical order, duplicates, or non-canonical list/set encoding and MUST NOT sort, deduplicate, or otherwise normalize submitted values.

Actor composition is asynchronous and provides no same-block or one-event-one-cycle guarantee. `SourceFilter::Any` permits any certified positive movement that is not self/no-op and satisfies the authored `AssetFilter`, independently of funding acceptance, to set `pending_signal`. Actors charges no signaller, requires no accepted funding, and imposes no trigger-specific minimum amount; any resulting User attempt charges the actor's fee budget.

User Actors MAY form externally closed self-cycles and arbitrary multi-Actor cycles. Actors performs no User graph analysis, stores no topological rank, and imposes no authored cycle prohibition. The FIFO cutoff and one live latch/ticket prevent synchronous recursion and more than one execution per Actor per block; every admitted User attempt still pays its complete suffix fee envelope, and an unreplenished cycle closes through `BalanceExhausted` or `FeeBudgetExhausted`.

System-to-System activation policy is host-owned. Before an Active System Contract is installed or replaced, the pallet invokes the configured `SystemActorContractValidator` with the assigned Actor id and candidate Contract. The default validator accepts all contracts; a host MAY reject candidates against a bounded runtime-owned manifest. Host graph rank, edge metadata, and external-effect inventory are neither Actor Contract identity nor User authoring surface.

### 3.2 Triggers and Timing

| Trigger | Readiness |
| --- | --- |
| `Manual` | An admitted owner signal sets `pending_signal` |
| `AddressEvent` | One matching certified positive movement sets `pending_signal` |
| `ObservationChange` | One matching deferred feed publication sets `pending_signal` |
| `ObservationCrossing` | One armed directional boundary crossing sets `pending_signal`; rearm crossings change detection state only |
| `Cadenced` | One internal timestamp deadline materializes readiness without a latch |

A Trigger changes readiness only. Every detector converges through one activation operation that sets `pending_signal` `false -> true` and requests the same canonical placement contract. `pending_signal` is the sole external-signal latch. An AddressEvent without concrete source matches only `SourceFilter::Any`. One Actor never combines independent readiness sources.

`pending_signal` has no direct clear call. Fresh opening consumes it; deactivation deletes it. Mutable deactivation followed by activation is the only actor-control reset. An Immutable actor retains an accepted latch until opening or close.

Fresh Active installation does not set `pending_signal`. `Manual`, `AddressEvent`, `ObservationChange`, and `ObservationCrossing` remain unready until their one source matches. `every_ticks` defines eligibility, not execution frequency.

`ObservationCrossing` authored identity contains exactly `feed`, `direction`, `threshold`, and `rearm_threshold`. Rising requires `rearm_threshold < threshold`; Falling requires `rearm_threshold > threshold`. Equality is invalid. One shared semantic validator owns this rule for creation, activation, replacement, genesis, simulation expected-value validation, and runtime integrity.

An armed Rising crossing fires exactly when `previous < threshold && current >= threshold`; its disarmed state rearms exactly when `previous > rearm_threshold && current <= rearm_threshold`. An armed Falling crossing fires exactly when `previous > threshold && current <= threshold`; its disarmed state rearms exactly when `previous < rearm_threshold && current >= rearm_threshold`. Repeated equal observations cause neither transition.

Active installation, reactivation, or semantic replacement initializes Crossing state from the current canonical observation without retroactive activation. A Rising value already `>= threshold`, or a Falling value already `<= threshold`, starts disarmed and waits for rearm; every other valid current value starts armed. Unavailable or uninitialized current state rejects Active admission through a typed error. Dormant authoring remains valid because it establishes no derived detection membership.

One fire disarms the Crossing before requesting activation and atomically moves its derived membership to the rearm boundary. Queue pressure, delayed activation materialization, duplicate publication, and a set `pending_signal` cannot produce another fire before a qualifying rearm. Rearm requests no activation and atomically restores fire membership. Detection owns armed state; the canonical FIFO owns eventual execution.

Every committed canonical observation update owns a monotonically increasing feed revision and one exact `previous -> current` transition. A transition that could fire or rearm is either durably admitted for bounded ordered processing or the observation publication fails atomically. Implementations MUST NOT coalesce revisions when an intermediate reversal, fire, or rearm could be lost, and later revisions for one feed cannot overtake its earlier incomplete transition.

`ObservationChange` remains a separate broad trigger over every committed change. It may share the same revision identity with Crossing processing, but it retains its subscriber pages and MUST NOT be reinterpreted as a threshold crossing or optimized by dropping subscribers.

The reference cadence tick is 500 milliseconds of consensus timestamp. `now_tick = floor(timestamp_millis / 500)` decides readiness. Fresh activation uses `anchor_tick = ceil(timestamp_millis / 500)` and sets its first deadline to `anchor_tick + every_ticks`, so quantization never shortens the authored period. Genesis has no consensus timestamp: it stores an uninitialized anchor and one tick-zero bootstrap wakeup. The first ordinary wakeup service after timestamp inherent application sets the same ceiled anchor and full-period deadline without latching readiness or entering FIFO.

`Cadenced` requires `cooldown_blocks == 0` and no ScheduleWindow. This keeps each Actor in at most one temporal membership across the timestamp cadence index and the block-keyed retry/window substrate. A suspended Cadenced Actor temporarily uses only its ordinary block retry wakeup; after the cycle terminates, cadence rearms strictly after the observed tick.

Contract timing admission uses:

```rust
ensure!(
  contract.cooldown_blocks <= MaxExecutionDelayBlocks,
  Error::ExecutionDelayTooLong
);

if let Trigger::Cadenced { every_ticks } = contract.trigger {
  ensure!(every_ticks > 0, Error::InvalidTriggerConfiguration);
  ensure!(every_ticks <= MaxCadenceDelayTicks, Error::ExecutionDelayTooLong);
  ensure!(contract.cooldown_blocks == 0, Error::InvalidTriggerConfiguration);
  ensure!(contract.window.is_none(), Error::InvalidScheduleWindow);
}
```

A zero cadence returns `InvalidTriggerConfiguration`. A delay above its typed protocol horizon returns `ExecutionDelayTooLong`. `MaxCadenceDelayTicks` is measured in cadence ticks and never reused as a consensus-block bound; `MaxExecutionDelayBlocks` is measured in consensus blocks and never reused as a cadence bound. The reference values are independently derived ten-Julian-year horizons: 631,152,000 ticks at 500 milliseconds and 52,596,000 blocks at 6 seconds. Overflow of otherwise valid cadence arithmetic returns `SchedulerIndexExhausted`.

```text
cooldown_anchor =
  last_cycle_block.or(schedule_anchor)

cooldown_eligible_at =
  schedule_anchor
    if cycle_nonce == 0 && last_cycle_block is None;
  checked_add(cooldown_anchor, cooldown_blocks)
    otherwise
```

Cadence arithmetic is constant-time:

```rust
fn first_cadence_due_tick(timestamp_millis, tick_millis, every_ticks) {
  checked_add(ceil(timestamp_millis / tick_millis), every_ticks)
}

fn next_cadence_due_tick(anchor_tick, every_ticks, now_tick) {
  first_due = checked_add(anchor_tick, every_ticks);
  lower = checked_add(now_tick, 1);
  if lower <= first_due { return first_due; }
  periods = ceil(checked_sub(lower, anchor_tick) / every_ticks);
  checked_add(anchor_tick, checked_mul(periods, every_ticks))
}
```

Admission guarantees nonzero `tick_millis` and `every_ticks`. Readiness floors the current timestamp; activation anchors ceil it. Rearming chooses the first cadence point strictly after the observed tick, so delayed service coalesces missed points into one opportunity and never creates catch-up cycles.

```text
window_floor =
  schedule_window.start when a window exists;
  BlockNumber::zero() otherwise

signal_eligible_at(lower) =
  max(lower, cooldown_eligible_at, window_floor)
```

Classification for `Manual`, `AddressEvent`, `ObservationChange`, and `ObservationCrossing` uses block-number eligibility plus `pending_signal`. `Cadenced` uses only its current timestamp tick while Idle, and block retry eligibility while Suspended.

If `cycle_nonce == 0 && last_cycle_block == None`, cooldown begins at `schedule_anchor` without addition. Otherwise cooldown is added to `last_cycle_block.or(schedule_anchor)`.

Retry timing is:

```text
retry_eligible_at =
  checked_add(
    last_attempt_block,
    max(cooldown_blocks, backoff(unsuccessful_attempts_at_cursor - 1))
  )
```

Every Cadenced Actor with the same `anchor_tick` and period has the same temporal gates. FIFO admission and bounded service, not an authored or derived phase, determine execution order.

For window installation:

```text
start_delay =
  start.saturating_sub(now)

first_temporal_eligible =
  temporal_eligible_at(schedule_anchor) under the prospective Active state
```

Validation requires `end > start`, representable `end + 1`, inclusive length `>= MinWindowLength`, `now <= end`, `start_delay <= MaxExecutionDelayBlocks`, and `first_temporal_eligible <= end`. This ScheduleWindow contract applies only to `Manual`, `AddressEvent`, `ObservationChange`, and `ObservationCrossing`; no future signal is assumed.

### 3.3 Precondition and Amounts

An absent Step `precondition` is unconditional. A present `Precondition` is one non-empty disjunctive-normal-form value: outer clauses compose as OR and each inner clause composes as AND. JSON uses only singular `precondition`; `{"precondition":{"clauses":[[A, B], [C]]}}` means `(A AND B) OR C`. An empty outer array or any empty inner array is invalid, and no plural field alias is accepted.

Every predicate explicitly carries `ObservationTiming::{Opening, Current}`. No predicate has an implicit timing. `Opening` evaluates through the single predicate evaluator while opening the logical cycle and freezes the resulting truth value for every step, retry, and Continuation in that cycle. `Current` invokes the same evaluator immediately before its owning step and therefore sees successful effects committed by earlier steps in the same attempt. Any predicate error fails the precondition expression permanently; false yields `StepSkipped(PreconditionFalse)` and never creates failure, retry, or suspension.

Admission canonicalizes each clause by canonical typed SCALE order, removes repeated predicates within that clause, then orders clauses by their canonical typed SCALE encoding. Two clauses that become identical after predicate deduplication are rejected. After that rejection, a clause that is an exact predicate superset of another clause is absorbed by the subset under `A OR (A AND B) = A`. Canonical equality and exact no-op comparison use this admitted form before rate limiting, cancellation, storage writes, placement reconstruction, or events.

A present `Precondition` contains `1..=MaxPreconditionClauses` conjunctions, each conjunction contains `1..=MaxPredicatesPerClause` predicates, and the sum across all clauses is `1..=MaxPredicatesPerStep`. Production, continuation, simulation, fee estimation, Weight derivation, forecast, generated vectors, and client explanation consume one evaluator result type and visit every admitted predicate without short-circuit. The bounded full visit prevents data-dependent Weight while preserving ordinary AND/OR truth.

Every `Above` predicate uses strict `>` comparison. Every `Below` predicate uses strict `<` comparison. Equality satisfies neither.

Observation `Stale`, `Uninitialized`, or `Unavailable` is false. A structurally invalid `Fresh` yields `PredicateError::InvalidObservation` and is Permanent. `max_age_blocks > 0`. Predicates are pure.

The subject of every `Balance*` predicate is the actor's sovereign account. Predicates cannot read a third-party account. They use the ordinary `AssetOps::balance` surface. For `FeeNativeAssetId`, they subtract the current transient fee reservation but not the protected minimum. They never redirect to `StakingOps::share_balance`.

Predicate truth does not authorize spending; resolution independently enforces capacity.

Percentages use widened floor division. `PercentageOfCurrent` and `PercentageAtOpening` use the same policy-specific surface and differ only in read time. `PercentageAtOpening` never reads signal payload or AddressEvent amount.

| Policy | Tasks | Current/opening base and capacity |
| --- | --- | --- |
| preserve source | Transfer, SplitTransfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | Outside a terminal custody drain, `AllAvailable` and `PercentageOfCurrent` use `preservable_balance`; `PercentageAtOpening` reads `OpeningSurface::PreservableAsset`; fixed/opening/funding values must fit current preservable capacity. An admitted terminal custody drain uses `terminal_drain_balance` for its sole Transfer. |
| output target | Mint, SwapOut | `PercentageOfCurrent` uses `spendable_balance`; `PercentageAtOpening` reads `OpeningSurface::TargetAsset`; fixed/opening/funding target values are not capped by current target balance; `AllAvailable` is forbidden |
| share spend | Unstake | current basis is `StakingOps::share_balance`; `PercentageAtOpening` reads `OpeningSurface::StakingShares`; `AllAvailable` spends all current shares; fixed/opening/funding values must fit current shares |

```text
spendable_balance(actor, asset, reserved_fee_remaining) =
  AssetOps::balance(actor, asset).saturating_sub(reserved_fee_remaining)
    if asset == FeeNativeAssetId;
  AssetOps::balance(actor, asset)
    otherwise

protected_minimum(actor, asset) =
  MinUserBalance
    if actor is User and asset == FeeNativeAssetId;
  AssetOps::minimum_balance(asset)
    otherwise

preservable_balance(actor, asset, reserved_fee_remaining) =
  spendable_balance(actor, asset, reserved_fee_remaining)
    .saturating_sub(protected_minimum(actor, asset))

terminal_drain_balance(actor, asset, reserved_fee_remaining) =
  spendable_balance(actor, asset, reserved_fee_remaining)
```

`reserved_fee_remaining` is zero for System attempts.

A field yields:

- `Resolved(value)` for a positive valid value;
- `Skipped` for a valid dynamic zero, including zero current or opening basis;
- `FundingUnavailable` for absent/zero `PercentageOfLastFunding` basis, a positive debit above current source/share capacity, or a required auxiliary debit cap of zero.

Every admitted `PercentageAtOpening` surface exists in `opening_snapshot`, including zero-valued surfaces. A missing key is `SnapshotUnavailable` and indicates invariant failure.

Multi-surface aggregation:

```text
any FundingUnavailable  -> FundingUnavailable
else any Skipped        -> Skipped
else                    -> Executable(all resolved fields)
```

Admission rejects zero fixed/percentage/absolute bounds, `AllAvailable` on Mint/SwapOut, identical pairs, `Transfer.to == sovereign_account`, any split recipient equal to the sovereign account, duplicate split recipients, mismatched LP pair, unsupported/class-forbidden modes, and zero required liquidity minima. Self-recipient rejection uses `SelfTransferNotAllowed`.

Outside an admitted terminal custody drain, `AllAvailable` preserves the current protected minimum and reserves no fee budget beyond the current attempt. Inside an admitted terminal custody drain, `AllAvailable` resolves to `terminal_drain_balance`; successful transfer cannot leave the actor open because transfer and close share one scheduler-attempt transaction.

### 3.4 Tasks and Canonical Step Control

Task variants own their declared asset and recipient fields. Mint is System-only.

| Task | Debit / output contract |
| --- | --- |
| Transfer, Burn | debit exactly the resolved amount |
| Stake | stake exactly the resolved amount |
| Unstake | unstake exactly the resolved shares; `AllAvailable` first resolves to the full current share balance |
| SwapIn | debit exactly the resolved input; `DexOps` owns executable quote and output protection |
| SwapOut | credit exactly the resolved output; Actors supplies one finite input-cap ceiling and `DexOps` owns executable quote/tolerance cap |
| AddLiquidity | resolved `amount_a` and `amount_b` are debit caps; return exact positive used amounts and LP output meeting `min_lp_out` |
| RemoveLiquidity | debit exactly the resolved `lp_amount`; `min_amount_a` and `min_amount_b` are output floors |
| DonateLiquidity | `max_amount_a` and derived `max_amount_b` are debit caps; return exact positive used amounts within both caps |

Ordered LP identity is validated at admission and rechecked at execution. `RemoveLiquidity` MUST NOT perform a smaller partial LP debit than the resolved `lp_amount`.

An Actor Contract is a terminal custody drain exactly when every listed property holds:

- `completion_policy == CloseAfterProductiveCycle`;
- `contract_steps` contains exactly one step;
- That step has `precondition: None`;
- That step is `Transfer { to, asset, amount: AllAvailable }`;
- That step uses `AbortCycle`.

For a terminal custody drain only:

- The Transfer amount resolves to `terminal_drain_balance`, not `preservable_balance`;
- For `FeeNativeAssetId`, the complete current attempt reservation is excluded before resolution, and the attempted-step fee is collected before transfer;
- Successful Transfer completes the one-step plan with `committed_effectful_tasks == 1` and MUST close with `ProductiveCycleCompleted` in the same scheduler-attempt transaction;
- Failure after provisional Transfer success, including terminal-cleanup failure, rolls back the Transfer, fees, events, and close with the enclosing attempt;
- Transfer failure commits no Transfer or close and follows ordinary `AbortCycle` fee and failure semantics.
- If `terminal_drain_balance == 0`, the sole step follows ordinary zero-resolution semantics: it emits `StepSkipped(ResolutionSkipped)`, increments `skipped_resolution`, completes the cycle with no committed effectful task, does not close with `ProductiveCycleCompleted`, and retains the actor under ordinary post-cycle placement. Actors defines no empty-drain close reason.

A zero terminal drain under `Cadenced` may therefore open later empty cycles and charge their User evaluation fees. This is an explicit consequence of requiring productive close rather than an implicit empty-balance terminal path; authors that do not want recurring probes use signal-driven readiness or another lifecycle policy.

No other plan may debit `protected_minimum`. One terminal custody drain names one asset. Recovering multiple known assets requires repeated successful reattachment and terminal-drain cycles; Actors performs no custody asset scan.

For DonateLiquidity:

```text
max_amount_b =
  preservable_balance(actor, asset_b, reserved_fee_remaining)
```

A derived `max_amount_b == 0` contributes `FundingUnavailable`. The adapter receives both finite caps and `max_ratio_error`; it MUST NOT invent a larger cap. Dynamic pool ratio/liquidity/cap movement is Temporary; malformed pair or configuration is Permanent. `LiquidityDonated` records exact derived caps and actual used amounts.

`FundingUnavailable` is a pre-task amount-resolution outcome, not a task failure. For this outcome, `ContinueNextStep` and `AbortCycle` both emit `StepSkipped(FundingUnavailable)` and advance. Only `RetryLater` replaces that advancing result with suspension or unsuccessful-bound failure. An implementation MUST NOT apply the task-failure semantics of `AbortCycle` to `FundingUnavailable`.

The authoring meaning is explicit: `AbortCycle` aborts on task failure, not unavailable funding. Likewise, a cycle whose attempted tasks all fail under `ContinueNextStep` reaches `Completed`, resets `unsuccessful_attempt_streak`, and retains factual `failed_steps`; the streak counts unsuccessful attempts, not failed steps.

Each step whose enclosing scheduler attempt commits selects exactly one row of the following closed transition table. A rolled-back attempt persists no row:

| Row | Step result | Policy / bound state | Step event | `OutcomeTotals` delta | Transition and boundary event |
| --- | --- | --- | --- | --- | --- |
| `ST-01` | precondition false | any | `StepSkipped(PreconditionFalse)` | `precondition_skips += 1` | advance |
| `ST-02` | resolution `Skipped` | any | `StepSkipped(ResolutionSkipped)` | `skipped_resolution += 1` | advance |
| `ST-03` | `FundingUnavailable` | `ContinueNextStep` or `AbortCycle` | `StepSkipped(FundingUnavailable)` | `skipped_funding_unavailable += 1` | advance |
| `ST-04` | `FundingUnavailable` | `RetryLater`, neither unsuccessful bound reached | none | none | remain at cursor; apply one local/global unsuccessful increment; emit `CycleSuspended(FundingUnavailable)` |
| `ST-05` | `FundingUnavailable` | `RetryLater`, a local/global unsuccessful bound reached | none | none | finalize `Failed`; emit `CycleSummary(Failed)`, then close with the Section 2.4 bound reason |
| `ST-06` | successful effectful task | any | the task success event | `executed_steps += 1`; `committed_effectful_tasks += 1` | advance; at plan end finalize `Completed` |
| `ST-07` | successful `StopCycle` | any | `CycleStopped` | `executed_steps += 1` | finalize `Completed` immediately |
| `ST-08` | Temporary task failure | `ContinueNextStep` | `StepFailed(Temporary)` | `failed_steps += 1` | advance |
| `ST-09` | Temporary task failure | `AbortCycle` | `StepFailed(Temporary)` | `failed_steps += 1` | apply one global unsuccessful increment; finalize `Failed`; emit `CycleSummary(Failed)` and close with `ConsecutiveFailures` when that bound is reached |
| `ST-10` | Temporary task failure | `RetryLater`, neither unsuccessful bound reached | `StepFailed(Temporary)` | `failed_steps += 1` | remain at cursor; apply one local/global unsuccessful increment; emit `CycleSuspended(Temporary)` |
| `ST-11` | Temporary task failure | `RetryLater`, a local/global unsuccessful bound reached | `StepFailed(Temporary)` | `failed_steps += 1` | finalize `Failed`; emit `CycleSummary(Failed)`, then close with the Section 2.4 bound reason |
| `ST-12` | Permanent task failure | `ContinueNextStep` | `StepFailed(Permanent)` | `failed_steps += 1` | advance |
| `ST-13` | Permanent task failure | `AbortCycle` or `RetryLater` | `StepFailed(Permanent)` | `failed_steps += 1` | apply one global unsuccessful increment; finalize `Failed`; emit `CycleSummary(Failed)` and close with `ConsecutiveFailures` when that bound is reached |

A step increments at most one `skipped_*` counter. `CycleSuspended` and `CycleSummary` carry cumulative outcomes after the current row's delta. A step event precedes the boundary event caused by that step. Prior successful task layers and their outcome deltas remain provisional until the enclosing scheduler attempt commits. The cursor only remains, advances by one, or terminates.

For SwapOut:

```text
capacity_cap =
  preservable_balance(actor, asset_in, reserved_fee_remaining)

authored_cap =
  capacity_cap
    for InputLimit::LiveQuote;
  min(capacity_cap, absolute)
    for InputLimit::Absolute(absolute)
```

`authored_cap == 0` is `FundingUnavailable`. No Actors-side quote call exists.

Inside one `DexOps` task layer:

```text
ceil_perbill(x,p) =
  (wide(x) * p.deconstruct() + Perbill::ACCURACY - 1)
  / Perbill::ACCURACY

quoted_tolerance_cap =
  checked_add(
    quote_required_in,
    checked_narrow(ceil_perbill(quote_required_in, tolerance))
  )

effective_max_in =
  min(authored_cap, quoted_tolerance_cap)
```

The adapter returns Temporary before mutation when quote is unavailable/zero, `effective_max_in < quote_required_in`, or execution cannot satisfy exact output within `effective_max_in`. `InputLimit::Absolute` is a cap, not an admission gate.

For SwapIn, the adapter obtains one current executable quote before mutation and requires:

```text
actual_out > 0
actual_out >= floor((1 - tolerance) * quote_output)
```

`slippage_tolerance == Perbill::one()` is allowed but does not waive `actual_out > 0` or any finite SwapOut cap. Exact-input quotes are never inverted across balance width.

Quote, tolerance arithmetic, System guard, Router call, actual validation, paired ingress, rollback, and success event belong to one task Weight class and one task transaction. Actors exposes no competing quote surface.

### 3.5 SplitTransfer

Requires `2..=MaxSplitTransferLegs`, positive shares, unique non-self recipients, and sum `<= 1`.

```text
leg_i = floor(total * share_i)
distributed = checked_sum(leg_i)
retained = total - distributed
effective_legs = count(leg_i > 0)
```

`distributed == 0` emits `StepSkipped(ResolutionSkipped)`. Otherwise every nonzero leg preflights and commits in one task layer; `effective_legs == 1` is valid. Undeclared share and rounding dust remain on the sovereign account. Recipient deposit failure is Temporary `RecipientDepositUnavailable`.

## 4. Execution and Economics

### 4.1 Actor Classification and Attempt Sequence

One pure runtime function owns Active actor classification. Scheduler admission, permissionless sweep, actor-targeting expiry substitution, certified-ingress expiry handling, simulation, and eligibility MUST use its result. Absence and a valid Dormant identity are resolved by the caller before classification; a partially present Active partition set is an invariant error.

```rust
enum ActorExecutionPhase<BlockNumber> {
  GlobalCircuitBreaker,
  Paused,
  WaitingRetry(BlockNumber),
  WaitingTemporal(BlockNumber),
  WaitingSignal,
  Ready,
}

struct ActorClassification<BlockNumber> {
  terminal_reason: Option<CloseReason>,
  execution_phase: ActorExecutionPhase<BlockNumber>,
}

enum ActorClassificationError {
  ActorInvariant,
  ContinuationInvariant,
  ComputationOverflow,
}
```

These are internal semantic types. Public runtime APIs expose only the projections in Section 7.

Classification returns `Result<ActorClassification<BlockNumber>, ActorClassificationError>`.

`terminal_reason` and `execution_phase` are separate outputs. `Ready` means only that no execution-phase gate applies; it does not override `terminal_reason` or authorize an attempt when `terminal_reason` exists.

Classification rules:

1. Validate the Active identity, hot, contract, and funding partitions and their required cross-partition relations. Missing or inconsistent required Active state returns `ActorInvariant`.
2. Validate `cycle_state` against `ContinuationState`, including cursor range, retry-policy ownership, positive and representable counters, and suffix snapshot bounds. A counter that reaches a terminal threshold remains valid terminal state and is evaluated by rule 4. A Suspended actor without one valid Continuation, or an Idle actor with a Continuation, returns `ContinuationInvariant`.
3. Select `cursor = 0` for Idle or `cursor = ContinuationState.cursor` for Suspended.
4. Compute `terminal_reason` by Section 2.4 precedence from terminal predicates already true in current stored state. Select the first satisfied predicate and do not evaluate lower-precedence predicates. User fee-budget evaluation uses the full plan from cursor `0` for Idle or the current suffix from `cursor` for Suspended.

   `CycleNonceExhausted` and `AutoCloseNonceReached` are classification predicates only while Idle. A stored retry or failure counter at its terminal threshold is valid terminal state. `ProductiveCycleCompleted` and counter changes that would be caused only by the current attempt are not predicted.
5. Compute `execution_phase` in this order:
  1. `GlobalCircuitBreaker` when the global breaker is active;
  2. `Paused` when the actor is paused;
  3. `Ready` when `terminal_reason` exists and neither preceding gate applies; no retry, temporal, or signal arithmetic is then evaluated;
  4. For Suspended, `WaitingRetry(next_block)` when retry eligibility is later than the current block, otherwise `Ready`;
  5. For Idle Cadenced, `Ready` when `pending_signal == true`; otherwise `WaitingCadenceTick(due_tick)` carrying the exact actor-owned tick pointer, even when that tick is already due but the wakeup worker has not materialized it;
  6. For other Idle triggers, `WaitingBlock(next_block)` when block eligibility is later than the current block;
  7. For signal-driven Idle, `WaitingSignal` when block eligibility has opened and `pending_signal == false`;
  8. `Ready` otherwise.
6. Checked arithmetic required to classify the existing actor, including current fee-envelope rederivation and retry/block-temporal target computation, returns `ComputationOverflow` on failure. Cadence classification reads its exact live tick pointer and never computes a later period. A classification error MUST NOT be converted into a phase, `NotReady`, absence, or `Ready`.

`WaitingSignal` carries no block because the next matching signal may arrive in any future block or never arrive.

Scheduler admission applies the classification in this order:

1. When `execution_phase == GlobalCircuitBreaker`, defer without closing or attempting.
2. When `terminal_reason` exists, admit the complete terminal-cleanup Weight; defer if it does not fit, otherwise close transactionally.
3. When `execution_phase != Ready`, defer without mutation.
4. Continue to attempt admission only when `terminal_reason == None` and `execution_phase == Ready`.

Permissionless sweep closes when `terminal_reason` exists and otherwise performs no actor transition. It ignores `execution_phase`.

Actor-targeting control and certified ingress use the same classification for expiry substitution. They act only when `terminal_reason == Some(WindowExpired)`; every other terminal reason remains owned by scheduler admission or sweep and does not replace the requested control or ingress effect. They ignore `execution_phase` for this purpose. An authorized Mutable actor MAY therefore repair a non-window terminal condition before scheduler or sweep commits close; User Immutable has no such control path.

Simulation and eligibility apply only their interface-specific presence, expected-value, and mode checks, then project the canonical classification. They MUST NOT independently recompute terminal or execution-phase predicates.

A scheduler classification error preserves the live head, performs no actor mutation, and stops the pass as `InvariantStalledLiveHead`. Every dispatch and runtime-API caller preserves classification errors through the exact projection in Section 9.2.

The attempt sequence begins only after scheduler admission accepts `terminal_reason == None` and `execution_phase == Ready`:

1. Admit the complete current attempt Weight and, for User, the exact current attempt fee envelope.
2. Load Continuation inputs or prepare fresh opening and funding snapshots from pre-opening state.
3. For a fresh cycle, atomically increment `cycle_nonce`, consume the latch and funding accumulator, update opening clocks, and emit `CycleStarted`. For a retry, atomically increment `attempt` and emit `CycleContinued`.
4. For each suffix step:
  1. Evaluate the Step Precondition through all admitted Predicates;
  2. Resolve all amounts against the current reservation;
  3. Remove that step's complete envelope from the reservation exactly once;
  4. Collect the selected User fee; failure follows Section 4.3, bypasses `StepErrorPolicy`, and rolls back the enclosing attempt before task dispatch;
  5. Execute one nested task layer when the task is attempted;
  6. Select the Section 3.4 transition row; an advancing row continues, and a suspension or terminal row exits the loop.
5. Finalize the selected row or plan end: apply the Section 2.4 failure-counter transition, emit the Section 3.4/Section 8 boundary event, and determine any failure- or completion-driven close reason.
6. Close when finalization requires it; otherwise install at most one future scheduler path, or install none when no readiness is due.
7. Commit the scheduler-attempt transaction.

New cycles start at cursor `0`. Retries start at the Continuation cursor. `executions(actor_id, block) <= 1`.

### 4.2 Snapshots and Fresh Opening

Fresh-cycle read-only preparation occurs after fee reservation and before `CycleStarted`.

```text
opening_snapshot =
  every unique policy-specific OpeningSurface referenced by
  PercentageAtOpening in contract_steps[0..]

proposed_funding_snapshot =
  funding_accumulated as observed before opening
```

Opening surfaces:

```text
PreservableAsset(asset) -> preservable_balance(actor, asset, reservation)
TargetAsset(asset)      -> spendable_balance(actor, asset, reservation)
StakingShares(asset)    -> StakingOps::share_balance(actor, asset)
```

The opening snapshot is independent of trigger kind, signal payload, and funding tracking. Every admitted surface is present even when zero. Contract admission computes the exact unique surface set, requires every share mapping to exist, and returns `AdmissionBoundOverflow` rather than truncating when the set exceeds `MaxOpeningSnapshotEntries`. The snapshot contains no sender, event amount, or event list.

After every fallible opening check succeeds, one scheduler-attempt transaction atomically performs:

```text
next_cycle_nonce = checked_add(stored_cycle_nonce, 1)
stored_cycle_nonce = next_cycle_nonce
pending_signal = false
last_cycle_block = now
funding_accumulated = empty
emit CycleStarted { cycle_nonce: next_cycle_nonce }
```

The first step reads the exact opening snapshot and pre-clear funding snapshot. Failed pre-opening admission consumes neither latch nor accumulator and emits no `CycleStarted`. A signal accepted after opening may latch the next cycle but never changes open snapshots.

Retry performs no new capture. Suspension persists:

```text
opening_snapshot restricted to OpeningSurface keys referenced by
  PercentageAtOpening in contract_steps[cursor..]

funding_snapshot restricted to entries present in proposed_funding_snapshot and
  referenced by PercentageOfLastFunding in contract_steps[cursor..]
```

The unresolved cursor step is part of the suffix. The cursor never decreases. An absent funding entry denotes no accepted funding at opening; a missing admitted opening key is `SnapshotUnavailable`.

### 4.3 User Fees and Economic Apoptosis

```text
eval_weight_upper_i =
  step_evaluation_weight_upper(User, step_i)

eval_fee_i =
  WeightToFee(eval_weight_upper_i)

exec_fee_upper_i =
  WeightToFee(task_weight_upper(User, task_i))

step_envelope_i =
  checked_add(eval_fee_i, exec_fee_upper_i)

attempt_fee_envelope(plan, cursor, User).total =
  checked_sum(step_envelope_i for i in cursor..plan.len)

attempt_fee_envelope(_, _, System).total =
  0
```

`step_evaluation_weight_upper` includes complete Predicate evaluation, opening/current/funding amount preparation, fee collection, and the largest non-task step event reachable before task dispatch. It comes from the same generated `WeightInfo` as execution. No `StepBaseFee`, legacy condition-read fee, parallel Task-weight table, or client-owned numeric fee model may exist.

One `attempt_fee_envelope(plan, cursor, class)` is used by economic viability, admission, reservation, execution, and simulation.

For a User attempt beginning at cursor `c`:

```text
R_c = attempt_fee_envelope(plan, c, User).total

for each visited step i:
  conditions_i and amount_resolution_i read R_i

  charged_i =
    eval_fee_i
      if the task is not attempted;
    step_envelope_i
      if the task is attempted

  R_(i+1) =
    checked_sub(R_i, step_envelope_i)
```

The full Step envelope leaves reservation exactly once regardless of Precondition truth, resolution, failure class, or charged amount. The decrement occurs after that Step's Precondition/resolution and before any later Step's Precondition/resolution.

After a visited step, its full envelope is removed from reservation before any later step is resolved.

Skip and pre-task resolution failure charge evaluation only. A task attempt charges the combined upper bound, not measured actual Weight. Adapter failure retains that combined fee only if the enclosing scheduler attempt commits. Retry creates a new suffix envelope and charges again.

Successful User attempt admission proves that the payer holds `MinUserBalance` plus the complete current suffix fee envelope before attempt mutation. The reservation model prevents every ordinary task debit from consuming any fee amount still required by the current attempt. A conforming `FeeCollector::collect_fee` therefore MUST NOT fail solely because the payer lacks the reserved fee amount.

Failure of `FeeCollector::collect_fee` is not a step outcome or `TaskFailure`. It denotes failure of the fee-asset ledger movement, FeeSink deposit capability, fee-path configuration, or an invariant. Fee collection MUST remain ledger-only and MUST NOT invoke Actors ingress preflight or notification, accumulate funding, latch readiness, or create scheduler placement.

Failure MUST roll back the complete scheduler-attempt transaction and MUST NOT invoke `StepErrorPolicy`. It persists no head consumption, fee, event, counter, nonce, cursor, snapshot, or task effect. It MUST NOT map to `InsufficientFee`, `BalanceExhausted`, or `FeeBudgetExhausted`, be represented as `TaskFailure`, or be processed by `StepErrorPolicy`.

Actor service stops as `FeeCollectionStalledLiveHead`. Unused reservation is not charged; durable fees are never refunded.

A User actor is economically viable only while its protected balance and current attempt envelope satisfy Section 2.4. When scheduler or sweep observes nonviability, it closes through `BalanceExhausted` or `FeeBudgetExhausted`; an open Continuation is cancelled first. This does not grant control authority to User Immutable. A terminal custody drain remains subject to the same opening viability check; after the attempted-step fee is collected, its sole Transfer may consume the remaining protected minimum because productive close is mandatory in the same scheduler-attempt transaction.

User creation charges `ActorCreationFee` to the signed caller/owner inside the control transaction. The sovereign account never pays the creation fee. Inability to collect returns `InsufficientFee`; rejected creation restores every fee effect. System creation, System attempts, and terminal cleanup charge no Actors execution fee. Task-defined asset effects and Router fees still apply.

### 4.4 Atomicity

- **Task layer**: adapter movement, paired ingress, and success event provisionally commit or roll back together. A successful task layer becomes durable only when the enclosing scheduler attempt commits.
- **Control transaction**: identity/locator, lifecycle, contract/funding, Continuation, subscriptions, readiness, membership, counters, creation fee, events, and required placement commit together. `Err` is a rejected transition. Exact no-op mutates/emits nothing; expiry substitutes atomic close.
- **Scheduler-attempt transaction**: head consumption, opening/retry, nested task layers, finalization, and future placement/close commit together. Late capacity, namespace, or invariant failure rolls back the complete attempt, including tasks and fees.
- **Terminal cleanup**: preflight ownership/index consequences, then delete/repair without task, fee, balance movement, retry, or requeue. In a terminal custody drain, the Transfer occurs in the preceding task layer; cleanup itself remains balance-neutral.

Actors uses explicit storage transactions; FRAME dispatch failure alone does not provide the required rollback semantics.

## 5. Scheduler and Reactive Ingress

### 5.1 FIFO and Temporal Readiness

One logical FIFO owns ordinary readiness. Each actor has at most one live ticket; stale physical entries have no authority. After bounded temporal and observation work, `on_idle` snapshots `cutoff = NextQueueTicket`; only older tickets execute.

Readiness `<= now` uses FIFO; later readiness remains an exact temporal target. A live ticket materialized before the cutoff MAY execute in the same block subject to FIFO order, the one-execution guard, and available Weight. A ticket created at or after the cutoff is necessarily `>= cutoff` and MUST NOT execute in that actor-service pass. Post-cutoff readiness for `now + 1` MAY therefore use a ticket only for a later pass. Queue saturation preserves readiness through an exact next-block target.

At any actor state, at most one ordinary temporal target exists. It is either block-keyed or timestamp-tick-keyed, never both.

```rust
let ordinary_target = if actor.lifecycle == ActiveLifecycle::Paused {
  None
} else {
  match (actor.cycle_state, actor.trigger, actor.pending_signal) {
    (CycleState::Suspended, _, _) =>
      Some(Block(retry_eligible_at(actor)?)),
    (CycleState::Idle, Trigger::Cadenced { .. }, _) =>
      Some(Tick(next_cadence_due_tick(actor)?)),
    (CycleState::Idle, Trigger::Manual, true) |
    (CycleState::Idle, Trigger::AddressEvent { .. }, true) |
    (CycleState::Idle, Trigger::ObservationChange { .. }, true) |
    (CycleState::Idle, Trigger::ObservationCrossing { .. }, true) =>
      Some(Block(signal_eligible_at(actor, lower)?)),
    (CycleState::Idle, _, _) => None,
  }
};
```

A retry target and an Idle cadence target never coexist. A Paused actor has no ordinary target. Cadenced disallows ScheduleWindow, so timestamp cadence and a block terminal target never coexist. Signal-driven actors may select the earlier of their ordinary block target and `terminal_at`, retaining terminal precedence on equality.

A future target is represented by one typed `wakeup_pointer`; its domain, key, page, and slot identify the actor-owned physical membership exactly. Both bounded temporal indexes are owned by one coordinator and only materialize due readiness into the same FIFO. Neither index executes Actors directly.

Due temporal work removes the physical wakeup and materializes one FIFO service obligation. Actor classification later evaluates terminal predicates before ordinary readiness. Under the global breaker, materialization MAY occur but scheduler-owned close waits until the breaker clears.

Temporal insert, replacement, removal, earliest-due discovery, and materialization are bounded and scan neither empty block ranges nor all actors. Materialization and ownership removal are atomic.

After bounded stale cleanup, actor service admits the live head or stops; it never scans behind, retickets, or demotes it.

| Stop class | Meaning |
| --- | --- |
| `Empty` | no live eligible pre-cutoff entry remains after bounded stale cleanup |
| `Head` | canonical live pre-cutoff head is known |
| `WeightBlockedLiveHead` | the complete next discovery/service unit does not fit the meter |
| `FeeCollectionStalledLiveHead` | the runtime fee-collection path failed and the rolled-back live head remains retryable |
| `PassExhausted` | the bounded scan/attempt ceiling or same-block pass boundary ended this pass without invariant failure |
| `InvariantStalledLiveHead` | topology, ownership, Continuation, or transactional invariant prevents safe progress at a live head |

```rust
enum IdleStarvationPhase {
  Healthy,
  Starving { consecutive_blocks: u32 },
  Alerted { consecutive_blocks: u32 },
}
```

A semantically admitted actor cannot stall FIFO merely by using the maximum plan. Complete attempt Weight is known before mutation, one maximum attempt fits `ActorServiceReserve`, and an admitted attempt reaches a named bounded result. A maximum User attempt MAY consume the whole actor-service budget and be the only attempt in that block; it remains paid bounded service, not structural starvation.

`InvariantStalledLiveHead` therefore denotes malformed canonical state or transactional topology, not an expensive valid actor. It preserves the head and performs no actor mutation. Actors defines no call-level quarantine, demotion, reticketing, skip, or forced cleanup because those paths would create a second service order and repair authority. Persistent repair is available only through Section 9.4 for a deployed lineage, or through a fresh canonical genesis before one exists.

Fee-collection failure yields `FeeCollectionStalledLiveHead`, preserves the head, and retries only through later ordinary actor service. A starved actor-service pass is `WeightBlockedLiveHead`, `FeeCollectionStalledLiveHead`, or `InvariantStalledLiveHead` with no committed attempt. Starved passes saturating-increment `consecutive_blocks`; `PassExhausted` does not. Weight/pass deferral changes no actor state or event.

For every finite set of pre-cutoff tickets, recurring conforming reserve, finite stale churn, eventual placement capacity, conforming host interfaces, and no structural invariant failure imply eventual service in FIFO order. This liveness statement does not promise a fixed block latency or success of the authored economic task.

### 5.2 Observation Delivery

Broad subscriber pages derive only from `ObservationChange`. Runtime maintains bounded exact actor/feed and reverse ownership; one fanout unit addresses at most `ObservationPageSize` positions. `ObservationCrossing` instead derives sparse ordered fire/rearm membership from the same feed identity. Physical topology is implementation-owned and neither representation changes authored Trigger identity.

Total subscriptions are bounded by `MaxActiveActors`; each Actor owns at most one observation feed. Distinct subscribed feeds cannot exceed total subscriptions, and dirty obligations cannot exceed distinct subscribed feeds. No separate unbounded feed registry exists.

Creation/activation installs, schedule replacement diffs, and deactivation/close removes subscriptions inside the owning transaction. Installing the first subscriber infers no historical revision and creates no dirty obligation. Publication while a feed has no subscribers allocates no baseline or dirty state. The first later accepted changed publication with subscribers sets the baseline and creates one dirty obligation. Removing the final subscriber deletes both.

The generated certified-publisher inventory is the sole observation-publication authority. Each certified publisher owns the monotonically increasing revision sequence and exact previous/current scalar values for its feeds and calls `ObservationTransitionIngress::note_observation_transition(feed, transition)`. Actors validates but does not synthesize revisions or transition values.

Changed publication atomically maintains one highest accepted revision and one pending obligation per subscribed feed. Revision `0` or regression fails; equality is exact no-op; greater revision updates. Publication is O(1) and does not inspect subscriber groups, mutate actors, enqueue, evaluate, or execute. A path outside the certified inventory has no Actors observation effect.

Fanout runs before cutoff. One admitted unit reserves complete Weight, visits one bounded group, latches live actors, and ensures future placement before advancing durable progress. A pass snapshots one revision. Newer revisions update only latest revision and do not reset the current pass. Under recurring budget, eventual placement capacity, and finite subscription churn, each snapshotted pass completes, although the feed may remain dirty indefinitely. Completion means subscriber groups were visited, not actors executed.

### 5.3 Address-Event Ingress

Only a certified producer creates Actors AddressEvent semantics. The generated certified-producer inventory is the sole producer authority: it names every runtime movement path that claims Actors ingress and records one typed protocol, provenance, source availability, read-only preflight owner, consequence owner, atomic rollback owner, retry mapping, and Weight bound.

Actors Transfer, every SplitTransfer leg, and Mint are certified producers whenever their destination resolves to an Actors sovereign account.

| Protocol | Ordering | Atomicity owner |
| --- | --- | --- |
| `PostMovementNotify` | Read-only preflight, value movement, exactly one notify | Producer storage transaction |
| `BlockAtomicPostDispatch` | Read-only preflight, successful dispatch, exactly one post-dispatch notify | Block author/import state transaction; a notify rejection invalidates the candidate block |
| `XcmTransactionalPrecommit` | Read-only preflight, exactly one Actors precommit, consume and deposit the non-cloneable holding | Asset-transactor storage transaction |

`XcmTransactionalPrecommit` is the sole allowed pre-movement Actors mutation. It exists because `AssetsInHolding` is non-cloneable: successful deposit commits the precommit, while precommit or deposit failure restores every Actors effect, event, ledger entry, and the exact original holding. It is observationally equivalent at the transaction boundary to one successful movement plus one Actors consequence and MUST NOT be described as post-movement notification.

Every protocol begins with literal read-only preflight covering lifecycle, funding, trigger, and required placement. Any later failure restores movement and every Actors effect at the named atomicity boundary. Certification is atomic, not advisory: after preflight, a certified movement MUST NOT fall back to balance-only when its complete Actors consequence cannot commit.

A certified third-party movement to an Active actor MAY fail when its complete Actors consequence cannot commit. Recoverable queue/wakeup capacity or placement unavailability is Temporary. Monotonic ticket/index exhaustion, topology corruption, invalid provenance, and invariant failure are Permanent. Actors tasks preserve the classification through `TaskFailure`; non-Actors producers map it to their outer dispatch error.

A movement not named in the certified inventory is **balance-only**: it cannot latch readiness, update `funding_accumulated`, or emit `FundingAccumulated`, even when destination is an Actors sovereign account. It is not rejected by Actors scheduler pressure. No event scan, balance-diff scan, inbox, or implicit producer discovery is permitted.

A new balance-movement surface remains balance-only until the certified-producer inventory defines its typed protocol, preflight, consequence, rollback owner, failure classification, and Weight.

Additional rules:

- Absent or Dormant destination is balance-only.
- Zero or self/no-op movement creates no Actors ingress.
- Terminal handling follows Section 2.4 before funding/readiness processing. If notification closes the actor, credited value remains on the sovereign account.
- Funding acceptance follows Section 2.5. Untracked or policy-rejected credit is balance-only for funding but MAY independently match an AddressEvent trigger.
- Concrete source and typed provenance remain independent.
- Equal certified movements count separately for funding while readiness coalesces.
- Fee movement is outside the certified AddressEvent producer inventory. It commits only the ledger debit and FeeSink credit, never invokes Actors ingress, and remains independent from destination actor state and scheduler capacity. Downstream Fee Sink allocation follows its independently configured cadence.

### 5.4 Hooks, Breaker, and Reserved Actor Service

`on_initialize` performs no Actors work and returns zero Actors Weight.

`ActorOnIdleReserve` is an embedding guarantee: immediately before Actors `on_idle` in every conforming block, both remaining Weight dimensions are at least this value. Runtime MUST enforce the guarantee through block dispatch limits, mandatory-hook maxima, and hook order. Actors meters itself against `min(actual_remaining, ActorOnIdleReserve)` and runs before lower-priority idle consumers.

`on_idle` order:

```text
base
-> bounded saturated-FIFO stale cleanup
-> due temporal work
-> observation fanout
-> cutoff snapshot
-> FIFO attempts / scheduler-owned terminal cleanup when the breaker is clear
-> starvation update
```

`OnIdleBaseWeightUpper` covers entry, breaker read, fixed orchestration, cutoff snapshot, and maximum starvation-state branch. Temporal and observation worker bases live inside their own limits. FIFO discovery, consumption, attempt, final placement, and terminal cleanup live inside the admitted actor-service bound.

```text
ActorServiceReserve =
  ActorOnIdleReserve
  - OnIdleBaseWeightUpper
  - SaturatedQueueCleanupWeightUpper
  - WakeupWeightLimit
  - ObservationFanoutWeightLimit
```

Subtraction is checked component-wise. One maximum actor attempt including any reachable terminal cleanup, or one standalone terminal cleanup, MUST fit. `MaxExecutionsPerBlock` is only a count ceiling.

While the global breaker is active:

- FIFO attempts and scheduler-owned automatic terminal cleanup do not run;
- Bounded stale queue, temporal, and observation housekeeping continues;
- Creation, System locator reuse, and activation fail with `GlobalCircuitBreakerActive`;
- Otherwise authorized control over existing Mutable actors remains available subject to control-mutation limits;
- Manual latch, address/observation ingress, actor-targeting or ingress-triggered expiry close, explicit Mutable close, permissionless sweep, breaker control, and active-limit control remain available;
- No head is demoted or reticketed because FIFO discovery is not entered;
- Starvation accounting is frozen.

Clearing the breaker resumes ordinary FIFO service without changing preexisting order.

`permissionless_sweep` is O(1). `permissionless_sweep_many` is O(K), `K <= MaxSweepBatch`, and executes in one control transaction. It classifies Active actors only: terminal actors count as `closed`, nonterminal actors as `alive`, and absent or Dormant ids as `missing`. A classification or cleanup error rolls back the batch and its event; a malformed Active actor is never counted as `missing`.

Reaching `MaxIdleStarvationBlocks` enters `Alerted` and emits `IdleStarvationDetected` once. Each later starved pass updates the stored count without another detection event. A nonstarved actor-service pass returns to `Healthy` and emits `IdleStarvationRecovered` once if the prior state was `Alerted`. While the breaker is active no actor-service pass occurs and the count is unchanged. Starvation changes no breaker, priority, order, or execution authority.

## 6. Host Adapters and Weight

### 6.1 Failure Contract

```rust
enum RetryClass { Permanent, Temporary }

struct TaskFailure {
  error: DispatchError,
  retry: RetryClass,
}
```

Retryability never derives from strings, module indices, broad token errors, or raw coincidence. Unknown is Permanent.

Temporary includes dynamic slippage, authored-cap insufficiency against a current quote, reference freshness/deviation, liquidity/output/ratio movement, recipient deposit unavailability, and recoverable queue/wakeup capacity or placement unavailability reached through paired ingress.

Permanent includes unsupported/malformed configuration, invalid provenance, missing static capability, monotonic ticket/index namespace exhaustion, topology corruption, and invariant failure.

`FundingUnavailable` is a resolution outcome, not `TaskFailure`. `FeeCollector` failure is an enclosing-attempt infrastructure failure under Section 4.3, not a `TaskFailure` or `RetryClass`. Successful adapters remain within declared effects and debit surfaces.

### 6.2 Interfaces

```rust
struct ExecutionContext<'a, A> { actor: &'a A, actor_type: ActorType }
struct IngressFailure { error: DispatchError, retry: RetryClass }

trait AssetOps<A, I, B> {
  fn transfer(from: &A, to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn burn(who: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn mint(to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn balance(who: &A, asset: I) -> B;
  fn minimum_balance(asset: I) -> B;
  fn preflight_transfer(from: &A, to: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
}
trait DexOps<A, I, B> {
  fn swap_exact_in(context: ExecutionContext<'_, A>, asset_in: I, asset_out: I,
    amount_in: B, tolerance: Perbill) -> Result<B, TaskFailure>;
  fn swap_exact_out(context: ExecutionContext<'_, A>, asset_in: I, asset_out: I,
    amount_out: B, authored_input_cap: B, tolerance: Perbill) -> Result<B, TaskFailure>;
}
trait StakingOps<A, I, B> {
  fn stake(who: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
  fn unstake(who: &A, asset: I, shares: B) -> Result<(), TaskFailure>;
  fn share_balance(who: &A, asset: I) -> B;
  fn share_asset(asset: I) -> Option<I>;
}
trait LiquidityOps<A, I, B> {
  fn lp_assets(lp: I) -> Option<(I, I)>;
  fn add_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B, min_lp: B)
    -> Result<(B, B, B), TaskFailure>;
  fn remove_liquidity(who: &A, lp: I, a: I, b: I, lp_amount: B, min_a: B, min_b: B)
    -> Result<(B, B), TaskFailure>;
  fn donate_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B,
    max_ratio_error: Perbill) -> Result<(B, B), TaskFailure>;
}
trait FeeCollector<A, I, B> {
  fn collect_fee(payer: &A, sink: &A, fee_asset: I, amount: B) -> DispatchResult;
}
enum ScalarObservationState<N> {
  Unavailable, Uninitialized, Fresh { value: u128, observed_at: N }, Stale,
}
trait ObservationProvider<F, N> {
  fn observe(feed: &F, now: N, max_age: u32) -> ScalarObservationState<N>;
}
trait FundingAuthority<A> {
  fn permits(actor_id: ActorId, owner: &A, source: Option<&A>,
    provenance: Option<&FundingProvenance>) -> bool;
}
trait SovereignAccountPolicy<A> { fn is_reserved(account: &A) -> bool; }
trait AddressEventIngress<A, I, B> {
  fn preflight(event: &AddressEvent<A, I, B>) -> Result<(), IngressFailure>;
  fn notify(event: &AddressEvent<A, I, B>) -> Result<(), IngressFailure>;
}
struct ObservationTransition {
  revision: ObservationRevision, previous: Option<u128>, current: u128,
}
trait ObservationTransitionIngress<F> {
  fn note_observation_transition(feed: F, transition: ObservationTransition) -> DispatchResult;
}
```

`AssetOps.balance` is the pre-reservation ordinary balance surface. Mint is System-only; transfer preflight models withdrawal, recipient deposit, and any certified destination ingress consequence. Actors owns source preservation. `AssetOps::preflight_transfer` and `AssetOps::transfer` MUST permit an admitted terminal custody drain to consume the source minimum and MUST NOT impose a hidden preservation floor. Staking-share and ordered LP bindings are admitted Actor Contract identity, MUST NOT be reinterpreted in place, and change only through Section 9.4.

`DexOps` is an encapsulated quote-and-execute boundary. It obtains the current executable quote internally before mutation, applies Section 3.4 tolerance/cap formulas and Section 6.3 System guard, executes through Router, validates actual returned amounts, and returns actual output/input. No caller-visible quote method or stale caller-supplied quote exists.

Liquidity success uses positive amounts within supplied caps/minima and no undeclared debit. For DonateLiquidity, Actors derives `max_b`; `LiquidityOps` may choose actual amounts within both caps but MUST NOT derive or exceed another cap.

Valid `Fresh` requires `observed_at <= now` and age `<= max_age`; invalid Fresh is Permanent. Missing observation is `Unavailable`; missing funding authority denies. Every missing mutation capability fails closed.

### 6.3 System Swap Guard and Router Boundary

Actors supplies resolved intent, class, finite authored/custody caps, tolerance, and failure policy. `DexOps` owns current quote acquisition, quote protection, the Actors-specific System reference guard, Router invocation, and actual validation. Router owns route discovery, path validation, Router fees/protection, Oracle publication, execution, and route outcome. Actors neither defines nor alters route semantics.

For User context, `DexOps` MUST NOT apply System reference parameters. For System context, before mutation and again against returned actual amounts when they may differ from executable quote, it requires fresh nonzero directed reference values and enforces:

```text
exec_out * ref_in * Perbill::ACCURACY
  >=
(Perbill::ACCURACY - SystemSwapMaxReferenceDeviation.deconstruct())
  * ref_out
  * exec_in
```

All products use widened checked arithmetic. Better-than-reference execution passes. Missing/stale/zero reference or worse execution beyond the bound is Temporary and rolls back the task layer. Router does not own or reinterpret the System guard.

### 6.4 Weight and Fee Derivation

One generated `WeightInfo` owns calls, Task classes, Predicate/amount evaluation, fee collection, ingress, scheduler/observation units, probes, orchestration, Continuation, finalization, events, and cleanup.

Task Weight covers internal quote, tolerance arithmetic, System guard, Router/adapter work, paired ingress, rollback, actual validation, and success event. Transfer Weight upper-bounds ordinary preservation and the source-exhausting terminal custody-drain branch. A swap task bound covers every Router Weight class reachable through its adapter input.

Step evaluation Weight covers every Predicate, amount preparation, fee collection, and non-task outcome. Attempt Weight adds probes, opening snapshots, finalization, future placement or close, and cleanup. Composition overflow is `AdmissionBoundOverflow`. Simulation and fee conversion use the same authority.

Current bounds are derived on demand:

```text
cycle_weight_upper =
  derive_weight(actor_class, contract_steps, current Weight/adapter bindings)

cycle_fee_upper =
  derive_fee(actor_class, contract_steps, current WeightToFee bindings)

suffix_weight_upper(cursor) =
  derive_weight(actor_class, contract_steps[cursor..], current bindings)

suffix_fee_upper(cursor) =
  derive_fee(actor_class, contract_steps[cursor..], current bindings)
```

No attempt may admit, reserve, charge, simulate, or report from stored derived Weight or fee state.

A generated Weight or adapter change that would make a previously admitted semantic Weight class exceed its owning execution envelope MUST preserve that class's admission or apply Section 9.4 before ordinary execution resumes. Compatibility is determined from the complete finite semantic Weight-class domain, not by unbounded actor iteration. Actors defines no permanent generic cache-revalidation state.

`WeightToFee`, `MinUserBalance`, and fee changes induced by an admission-compatible `WeightInfo` update are economic-policy changes. They require no actor iteration and apply immediately to every User actor, including Immutable actors, unless a Section 9.4 migration defines a grace, subsidy, or alternate disposition. Any resulting close transitions with `BalanceExhausted` or `FeeBudgetExhausted`, including mass economic apoptosis, are canonical. System actors remain Actors-fee-exempt.

## 7. Calls and Runtime APIs

### 7.1 Calls

```text
create_user_actor                create_user_actor_at_slot
create_system_actor              create_system_actor_at_sovereign_id
activate_actor / deactivate_actor  pause_actor / resume_actor
manual_trigger                 close_actor
update_contract                cancel_continuation
set_global_circuit_breaker     set_active_actor_limit
permissionless_sweep           permissionless_sweep_many
```

Authorization:

| Call group | Required origin |
| --- | --- |
| User creation | signed caller, who becomes owner |
| System creation, System locator reuse, active-limit control | `SystemOrigin` |
| breaker control | `GlobalBreakerOrigin` |
| User actor control | signed owner |
| System actor control | signed owner or `SystemOrigin` |
| sweep | any signed origin |

`manual_trigger` requires Active, unpaused, nonexpired state and an authored Manual source. User Immutable MAY use it; System Immutable cannot author Manual. It sets `pending_signal` only `false -> true`, atomically ensures one future path, and emits `ManualTriggerSet` only for that transition. A previously latched actor with valid placement changes nothing.

When Suspended, Manual only latches readiness for the next logical cycle. It does not cancel, replace, accelerate, duplicate, or retarget the current retry path.

Actor-targeting control obtains the Section 4.1 classification before the control-mutation rate check. Complete Contract equality returns before expiry substitution, rate limiting, cancellation, clock mutation, placement reconstruction, writes, fees, or events. A non-equal replacement then applies expiry substitution and typed classification errors before mutation.

Canonical-equality pause/resume and unchanged breaker or active-limit values also return before mutation or event.

Calls that may close include cleanup Weight.

Active creation and activation respect `ActiveActorLimit`; all creation respects `MaxActorIdentities`; fresh System locator creation respects `MaxSystemSovereigns`.

User Active creation and User activation require the prospective/current sovereign fee-native balance to cover:

```text
MinUserBalance + attempt_fee_envelope(plan, 0, User).total
```

before creation fee or Active state commits. Failure returns `InsufficientBalance`. The unfunded lifecycle is `create Dormant -> fund deterministic sovereign account -> activate`.

User Dormant and Active creation charge `ActorCreationFee` to the signed caller/owner.

Auto-close `Some(t)` requires:

```text
1 <= t - cycle_nonce <= MaxAutoCloseNonceHorizon
```

Auto-close is changed only through complete Mutable Contract replacement; there is no field-specific setter or increment call.

Active-limit update requires nonzero:

```text
ActiveActorCount <= limit <= min(MaxActiveActors, MaxQueueLength)
```

System account derivation is a pure helper. No direct recovery-transfer call or close-time refund exists. Custody recovery uses fresh exact User-slot or vacant System-locator reattachment followed by a Section 3.4 terminal custody drain.

For User custody recovery after close:

1. The same signed owner calls `create_user_actor_at_slot` for the released exact slot, creating a fresh Mutable Dormant or Active actor;
2. The deterministic sovereign account is funded as required by ordinary Active admission;
3. A terminal custody drain transfers one named asset and closes the fresh actor;
4. The owner MAY repeat the sequence for other known assets.

Recovery of a non-fee asset requires prior funding of the same deterministic sovereign account with sufficient `FeeNativeAssetId` for Active admission and the current attempt. Actors provides no sponsored recovery path.

For System custody recovery, `SystemOrigin` reuses the same vacant locator and installs a fresh Mutable terminal custody-drain actor. Reattachment never restores the closed actor's semantics.

### 7.2 Simulation

```rust
enum SimulationMode { FreshCurrentPlan, CurrentContinuation }
enum AttemptDisposition { Completed, Failed, Suspended, Closed(CloseReason) }
enum StepOutcome {
  Executed, Stopped, Skipped(StepSkippedReason), FundingUnavailable,
  Failed(TaskFailure),
}
struct SimulationStepRecord { step_index: u32, outcome: StepOutcome }
enum SimulationError {
  TransactionDepthExceeded, Classification(ActorClassificationError), ActorNotFound,
  TypeMismatch, MutabilityMismatch, ContractMismatch, ModeCycleStateMismatch,
  GlobalCircuitBreaker, Paused, NotReady, FeeCollectionFailed,
}
struct SimulationResult {
  status: AttemptDisposition, cycle_nonce: u64, start_cursor: u32,
  continuation_cursor: Option<u32>, unsuccessful_attempts_at_cursor: Option<u32>,
  cumulative_outcomes: OutcomeTotals,
  steps: BoundedVec<SimulationStepRecord, MaxContractSteps>,
}
trait ActorSimulationApi<Contract> {
  fn simulate_current_contract(
    actor_id: ActorId, expected_type: ActorType, expected_mutability: Mutability,
    expected_contract: Contract, mode: SimulationMode,
  ) -> Result<SimulationResult, SimulationError>;
}
```

One call simulates exactly one fresh or Continuation attempt; `steps.len() <= contract_steps.len() - start_cursor <= MaxContractSteps`.

For an actor that passes Section 4.1 classification, mode ownership is exact:

| `SimulationMode` | Idle | Suspended |
| --- | --- | --- |
| `FreshCurrentPlan` | simulate one fresh attempt | `ModeCycleStateMismatch` |
| `CurrentContinuation` | `ModeCycleStateMismatch` | simulate one Continuation attempt |

Simulation resolves presence, invokes Section 4.1 once, applies expected type/mutability/contract checks and the mode matrix, then projects the result in this order:

1. A classification error maps exactly under Section 9.2.
2. Expected type, mutability, contract, or mode mismatch returns its interface error.
3. `GlobalCircuitBreaker` returns `SimulationError::GlobalCircuitBreaker`, including when `terminal_reason` exists.
4. When the breaker is clear and `terminal_reason` exists, return `Closed(reason)` with no step records.
5. `Paused` returns `SimulationError::Paused`.
6. `WaitingRetry`, `WaitingTemporal`, or `WaitingSignal` returns `SimulationError::NotReady`.
7. `Ready` with no terminal reason simulates one attempt. A fee-collection failure during that rollback-only attempt returns `SimulationError::FeeCollectionFailed` and no `SimulationResult`.

The canonical Step evaluator produces one `StepOutcome` before the authored error policy is interpreted. Production events/counters and rollback-only simulation consume that same value; simulation adds only the bounded `SimulationStepRecord` wrapper.

| Step transition | `StepOutcome` |
| --- | --- |
| successful effectful task | `Executed` |
| successful `StopCycle` | `Stopped` |
| precondition or zero resolution that advances | `Skipped(reason)` |
| `FundingUnavailable`, whether it advances, suspends, or reaches a bound | `FundingUnavailable` |
| task failure, whether it advances, suspends, or terminates | `Failed(TaskFailure { error, retry })` |

A failed outcome preserves the concrete `DispatchError` cause independently from `RetryClass`; the policy interpreter may use retry disposition but MUST NOT erase the cause. Fee-collection failure remains an enclosing-attempt infrastructure error and produces no committed Step outcome.

The shared `AttemptDisposition` distinguishes completion, suspension, terminal failure, and close for production finalization and simulation status. The simulated `cumulative_outcomes` is the exact production finalization value, including `failed_steps += 1` for a Temporary task failure whose disposition is `Suspended`; it is not reconstructed from trace records.

Attempt disposition mapping:

- `Completed`: successful cycle terminal and actor remains open.
- `Failed`: terminal `CycleResult::Failed` without suspension or close.
- `Suspended`: attempt would persist Continuation.
- `Closed(reason)`: current classification or attempt finalization would close with `reason`.

Result fields:

| Case | `cycle_nonce` | `start_cursor` |
| --- | --- | --- |
| fresh attempt | `stored_cycle_nonce + 1` | `0` |
| Continuation attempt | stored cycle nonce | stored cursor |
| pre-attempt close | stored cycle nonce | stored cursor or `0` |

`continuation_cursor` and `unsuccessful_attempts_at_cursor` are present iff the simulated result is `Suspended`. `cumulative_outcomes` and `steps` describe the simulated transition. A pre-attempt close has no step records.

Error precedence:

```text
TransactionDepthExceeded
ActorNotFound
ActorInvariant
ContinuationInvariant
ComputationOverflow
TypeMismatch
MutabilityMismatch
ContractMismatch
ModeCycleStateMismatch
GlobalCircuitBreaker
Paused
NotReady
FeeCollectionFailed
```

`ActorNotFound` includes a valid Dormant identity because no Active Actor Contract exists. Partial Active partitions return `ActorInvariant`; malformed Continuation returns `ContinuationInvariant`. Old `expected_contract` after semantic replacement returns `ContractMismatch`. Current contract with `CurrentContinuation` after cancellation returns `ModeCycleStateMismatch`.

Simulation executes the production attempt path in rollback-only storage, uses the shared predicate, amount, fee, task, `StepOutcome`, policy interpretation, counters, and `AttemptDisposition`, and persists no state/event. Simulation-specific state is limited to rollback/non-persistence, bounded trace records, explicit transaction-depth failure at the rollback boundary, and requested mode checks. `FeeCollectionFailed` is interface-local and is not an authored step failure.

### 7.3 Eligibility

```rust
enum ActorEligibility<BlockNumber> {
  NotRegistered,
  Dormant,
  Active(ActorClassification<BlockNumber>),
}
trait ActorEligibilityApi<BlockNumber> {
  fn actor_eligibility(actor_id: ActorId)
    -> Result<ActorEligibility<BlockNumber>, ActorClassificationError>;
}
```

Eligibility resolves absent and valid Dormant ids before invoking Section 4.1. For an Active actor, a classification error maps exactly under Section 9.2 and success returns the canonical `ActorClassification` without stripping terminal reason or execution-phase payloads. `WaitingRetry(block)` and `WaitingTemporal(block)` retain their exact block; no parallel next-block field exists.

Simulation evaluates one attempt at the current block. Eligibility exposes current classification. Neither predicts a future signal.

## 8. Events and Ordering

```rust
ActorCreated { actor_id: ActorId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle }
ActorActivated { actor_id: ActorId }
ActorDeactivated { actor_id: ActorId }
ActorPaused { actor_id: ActorId }
ActorResumed { actor_id: ActorId }
ActorClosed { actor_id: ActorId, reason: CloseReason }

CycleStarted { actor_id: ActorId, cycle_nonce: u64 }
CycleSummary { actor_id: ActorId, cycle_nonce: u64, result: CycleResult, outcomes: OutcomeTotals }
CycleSuspended { actor_id: ActorId, cycle_nonce: u64, cursor: u32, reason: SuspensionReason, cumulative_outcomes: OutcomeTotals }
CycleContinued { actor_id: ActorId, cycle_nonce: u64, cursor: u32 }
CycleCancelled { actor_id: ActorId, cycle_nonce: u64, reason: CancellationReason }
CycleStopped { actor_id: ActorId, cycle_nonce: u64, step_index: u32 }

StepSkipped { actor_id: ActorId, cycle_nonce: u64, step_index: u32, reason: StepSkippedReason }
StepFailed { actor_id: ActorId, cycle_nonce: u64, step_index: u32, retry_class: RetryClass, error: DispatchError }

TransferExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance, to: AccountId }
SplitTransferExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, total: Balance, distributed: Balance, retained: Balance, legs: u32, effective_legs: u32 }
SwapExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance }
BurnExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
MintExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
StakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
UnstakeExecuted { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset: AssetId, shares: Balance }
LiquidityDonated { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, max_amount_a: Balance, max_amount_b: Balance, amount_a: Balance, amount_b: Balance }
LiquidityAdded { actor_id: ActorId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance, lp_minted: Balance }
LiquidityRemoved { actor_id: ActorId, cycle_nonce: u64, step_index: u32, lp_asset: AssetId, lp_amount: Balance, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance }

ContractUpdated { actor_id: ActorId }
ActiveActorLimitSet { old_limit: u32, new_limit: u32 }
GlobalCircuitBreakerSet { paused: bool }
ManualTriggerSet { actor_id: ActorId }
FundingAccumulated { actor_id: ActorId, asset: AssetId, added: Balance, accumulated: Balance }
SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 }
IdleStarvationDetected { consecutive_blocks: u32 }
IdleStarvationRecovered { consecutive_blocks: u32 }
```

Exact field order and complete task-event fields come from metadata.

Event durability follows Section 4.4. `ManualTriggerSet` emits only on latch `false -> true`. `FundingAccumulated` emits only for tracked policy-accepted credit. Cycle key is `(actor_id, cycle_nonce)`; attempt adds `attempt`.

Active creation emits exactly one `ActorCreated { initial_lifecycle: Active }` and does not emit `ActorActivated`. `ActorActivated` belongs only to Dormant -> Active transition of an existing identity.

Pause/resume exact no-op emits no event.

Opening emits `CycleStarted` before every step event. Retry begins with `CycleContinued`, emits no second start, and consumes no second snapshot. Within an attempt, the step event in Section 3.4 precedes any boundary event caused by that step.

Exact boundary ordering is:

- An advancing skip or failure emits only its `StepSkipped` or `StepFailed` event before the next visited step;
- `FundingUnavailable` selected for `RetryLater` emits no `StepSkipped`; it emits `CycleSuspended(FundingUnavailable)`, or `CycleSummary(Failed)` then `ActorClosed(bound_reason)` when an unsuccessful bound is reached;
- Temporary task failure selected for `RetryLater` emits `StepFailed`, then `CycleSuspended(Temporary)`, or `CycleSummary(Failed)` then `ActorClosed(bound_reason)` when an unsuccessful bound is reached;
- Terminal task failure emits `StepFailed`, then `CycleSummary(Failed)`, then any failure-driven `ActorClosed`;
- Successful completion emits `CycleSummary(Completed)`, then any productive, auto-close, or post-attempt scheduler-index `ActorClosed`;
- A successful terminal custody drain emits `CycleStarted`, `TransferExecuted`, `CycleSummary(Completed)`, then `ActorClosed(ProductiveCycleCompleted)`;
- A zero terminal custody drain emits `CycleStarted`, `StepSkipped(ResolutionSkipped)`, then `CycleSummary(Completed)` and no productive close;
- Fee-collection failure rolls back the complete attempt and emits nothing.

`CycleSuspended` and `CycleSummary` expose cumulative outcomes after the current step delta. Cancellation emits `CycleCancelled`, then `CycleSummary(Cancelled)`. If caused by close, `ActorClosed` follows. If caused by semantic update or deactivation, the corresponding event follows. Pure close emits only `ActorClosed`. Weight/pass deferral emits nothing. Unbounded history is off-chain.

## 9. ABI, Errors, Storage, and Upgrades

### 9.1 ABI Authority

Sections 1-8 own semantic meaning. Exact indices, discriminants, field order, bounds, and runtime API encoding come only from complete generated metadata.

Metadata MUST expose only canonical shapes; reserved compatibility indices, stale aliases, dual names, and compatibility storage are forbidden. Canonical terminology is `Cycle`, `CloseAfterProductiveCycle`, and `ProductiveCycleCompleted`.

After a `1.0.0` stability declaration, existing public shapes are immutable; additions append or use a new typed wrapper.

### 9.2 Errors

```rust
enum Error {
  ActorIdOverflow, ActorNotFound, ActiveActorCapacityExceeded, ActiveActorCountInvariant,
  ActorIdentityCapacityExceeded, ActorIdentityCountInvariant, ActorInvariant,
  ActorAlreadyActive, ActorDormant,
  ActiveActorLimitExceedsQueueCapacity, ActiveActorLimitTooHigh,
  ActiveActorLimitTooLow, ActiveActorLimitBelowCurrent,
  ActorPaused, EmptyContractSteps, ContractStepsExceedOnIdleBudget,
  ExecutionDelayTooLong, GlobalCircuitBreakerActive, ImmutableActor,
  InsufficientBalance, InsufficientFee,
  InvalidAmountResolution, InvalidPredicate, InvalidAutoCloseNonce,
  InvalidScheduleWindow, InvalidSplitTransfer, InvalidTriggerConfiguration,
  InvalidTradeBound, InvalidRetryAttemptLimit, InvalidObservationMaxAge,
  SelfTransferNotAllowed, MintNotAllowedForUserActor,
  NotGovernance, NotOwner,
  OwnerSlotCapacityExceeded, OwnerSlotOccupied, InvalidOwnerSlot, ActorIdOccupied,
  SystemSovereignCapacityExceeded, SystemSovereignUnknown,
  SystemSovereignOccupied, SystemSovereignInvariant,
  SovereignAccountCollision, ReservedSovereignAccount,
  TooManyContractSteps, SnapshotUnavailable, FundingAccumulatorOverflow,
  QueueTicketExhausted, SchedulerIndexExhausted,
  AutoCloseNonceHorizonExceeded, ControlMutationRateLimited, QueueCapacityUnavailable,
  RetryLaterNotAllowedForImmutableActor, ContinuationNotFound, ContinuationInvariant,
  ComputationOverflow, EmptyPrecondition, ManualSourceDisabled,
  RecipientDepositUnavailable,
  ObservationSubscriptionCapacityExceeded, ObservationSubscriptionInvariant,
  InvalidObservationRevision, DirtyObservationCapacityExceeded,
  DirtyObservationInvariant, ObservationUnavailable, ObservationUninitialized,
  CrossingIndexCapacityExceeded, CrossingIndexInvariant, CrossingGenerationExhausted,
  CrossingTransitionCapacityExceeded, CrossingTransitionInvariant,
  SystemActorTopologyInvalid, AdmissionBoundOverflow,
}
```

Section 4.1 classification errors have one exact projection:

| `ActorClassificationError` | Dispatch `Error` | `SimulationError` |
| --- | --- | --- |
| `ActorInvariant` | `ActorInvariant` | `Classification(ActorInvariant)` |
| `ContinuationInvariant` | `ContinuationInvariant` | `Classification(ContinuationInvariant)` |
| `ComputationOverflow` | `ComputationOverflow` | `Classification(ComputationOverflow)` |

Eligibility returns `ActorClassificationError` directly without an interface-local duplicate. Simulation wraps that exact core once and adds only simulation-specific failures.

Every dispatch path that invokes classification MUST preserve this mapping. `permissionless_sweep_many` rolls back its complete transaction on a classification error. Certified ingress carries the corresponding pallet `Error` inside Permanent `IngressFailure`. A classification error MUST NOT map to `ActorNotFound`, `ActorDormant`, `NotReady`, `SchedulerIndexExhausted`, `AdmissionBoundOverflow`, or a waiting phase.

`ActorInvariant` belongs only to a malformed existing Active actor whose required identity, hot state, contract, funding state, or cross-partition relation is missing or inconsistent. An absent id and a valid Dormant identity use their caller-specific ordinary result and are not actor invariants.

`ContinuationInvariant` belongs to disagreement between `cycle_state` and `ContinuationState`, or to malformed Continuation cursor, policy, counter, snapshot, or suffix state.

`ComputationOverflow` belongs only to checked arithmetic required to classify an existing Active actor, including current fee-envelope rederivation and retry/temporal target computation. Contract creation or semantic replacement uses `AdmissionBoundOverflow`; funding accumulation, scheduler placement, auto-close mutation, and other call-specific transitions retain their own errors.

Resolution outcomes are not pallet errors. `TaskFailure.error` MAY carry a pallet error only as a stable execution diagnostic, without converting the step into an extrinsic-level rejection.

`RecipientDepositUnavailable` is Temporary only in that execution-time role. `InsufficientFee` belongs only to User creation-fee collection. `FeeCollectionFailed` belongs only to rollback-only simulation of an otherwise Ready User attempt whose runtime fee-collection path fails; production preserves the head as `FeeCollectionStalledLiveHead`. `ActorIdOccupied` belongs only to creation against already-owned `NextActorId`. `NotGovernance` applies when an origin accepted by `SystemOrigin` targets a User actor. Unknown adapter failure remains Permanent.

The error ABI contains no compatibility-only variant.

### 9.3 Storage Contract

Generated storage descriptors own exact prefixes, hashers, keys, values, and physical topology.

Normatively required:

- Section 2 canonical partitions;
- Persistent non-optional identity control clock;
- Exact slot/locator/reverse-index/subscription/readiness ownership;
- One live ticket and at most one ordinary temporal target per actor, subject to terminal coexistence;
- Bounded collections and exact counters/reverse ownership;
- Canonical ordered encoding;
- No unbounded execution history;
- No stored Weight/fee cache, cache epoch, cache-revalidation workset, parallel fee table, or writable composite actor;
- `try_state` reconciliation of ownership, cardinality, membership, revision, Continuation, terminal marker, tracked funding derivation, and processing bounds.

Initial state contains no migration, dual write, legacy decoder, bridge, stale alias, or compatibility storage. It initializes nonzero `ActiveActorLimit`, validates `SystemSovereignCount <= MaxSystemSovereigns`, sets a representable `NextActorId` above every configured actor id and reserved System locator id, and reconciles every configured System actor/custody locator, identity counter, reverse index, subscription, and initial scheduler path.

### 9.4 Deployed-Lineage Runtime Upgrade Migration

This section applies only after a host runtime has established persisted production lineage. Before first production genesis, a fresh canonical baseline MAY replace storage or ABI without migration ceremony, historical decoders, or compatibility aliases.

For a deployed lineage, each storage change, semantic state rewrite, incompatible Weight/admission change, or runtime-upgrade override of an Immutable actor first ships a migration-specific specification defining:

- Source and target schemas/semantics;
- Bounded work unit and durable progress owner;
- Temporary execution gate;
- Weight per invocation;
- Failure, interruption, resume, and completion behavior;
- Semantic mapping and actor disposition;
- Custody consequences;
- Terminal invariant;
- Continuation `Cancel | PreserveWithProof` policy;
- Storage-version transition and idempotence.

Idempotence means that after target completion, later invocations perform no semantic mutation. Partial migration cannot expose reinterpreted state to ordinary execution. Continuation preservation proves equivalent contract, frozen inputs, failures, fees, Weight, and eligibility; otherwise cancel.

A deployed-lineage migration that repairs a persistent structurally caused `InvariantStalledLiveHead` is the sole authority to rewrite or remove the malformed state. It MUST define actor disposition, custody preservation, queue ownership correction, FIFO consequences, and the exact service-resumption point.

A deployed-lineage change to sovereign-account derivation or reserved-account policy that could alter existing custody or reattachment is a semantic rewrite and MUST define the same custody disposition.

Migration state is specific to the concrete upgrade and MUST be absent or inert after completion. Actors defines no permanent generic migration subsystem.

## 10. Runtime Configuration

### 10.1 Relations

Required bindings include `ActorsPalletId`, `FeeNativeAssetId`, `SystemOrigin`, `GlobalBreakerOrigin`, adapters/services, `FeeSink`, `ActorCreationFee`, `WeightToFee`, one `WeightInfo`, `ActorOnIdleReserve`, generated base/cleanup bounds, `MaxSweepBatch`, independent queue/wakeup/observation processing bounds, System reference parameters, and `TargetBlockTime`.

1. `0 < ActiveActorLimit <= min(MaxActiveActors, MaxQueueLength)`; after update, `ActiveActorLimit >= ActiveActorCount`.
2. `MaxActorIdentities >= MaxActiveActors`.
3. `MaxSystemSovereigns > 0`; every count/index type represents its configured bound.
4. `0 < MaxOwnerSlots <= 255`.
5. `0 < MaxContractSteps <= 255`.
6. `MaxRetryAttempts >= 2`.
7. `MaxContractSteps * MaxRetryAttempts <= u32::MAX`, checked; this bounds every `OutcomeTotals` counter within one cycle.
8. `MaxConsecutiveFailures > 0`.
9. `MaxOpeningSnapshotEntries == 2 * MaxContractSteps` and `MaxOpeningPredicateResults == MaxContractSteps * MaxPredicatesPerStep`, checked; persisted funding entries fit `MaxFundingTrackedAssets`.
10. Observation subscriptions and dirty obligations obey the `MaxActiveActors` bound in Section 5.2.
11. `QueuePageSize`, `WakeupPageSize`, and `ObservationPageSize` are independently named and nonzero.
12. `MaxSweepBatch > 0`.
13. Every collection, scan, attempt, worker, and processing-unit bound is nonzero and has one owner.
14. Each worker limit covers one complete worst-case unit.
15. Runtime guarantees `remaining_weight_at_ACTORS_on_idle >= ActorOnIdleReserve` in both dimensions.
16. Section 5.4 subtraction is representable and `ActorServiceReserve` covers one maximum actor attempt including reachable terminal cleanup, or one standalone terminal cleanup.
17. Every admitted plan/suffix, producer consequence, control transition, and cleanup fits its owning envelope.
18. `MinUserBalance >= AssetOps::minimum_balance(FeeNativeAssetId)`.
19. `ActorCreationFee > 0`.
20. `WeightToFee` maps every admitted User evaluation/task upper bound to a positive fee.
21. `SystemSwapEmaMaxAgeBlocks > 0`; `SystemSwapMaxReferenceDeviation < Perbill::one()`.
22. `TargetBlockTime > 0`.
23. `MaxExecutionDelayBlocks = ceil(10 Julian years / TargetBlockTime)` and `MaxCadenceDelayTicks = ceil(10 Julian years / CadenceTickMillis)`; each horizon is nonzero, typed in its own clock, and its arithmetic is representable.
24. `MaxAutoCloseNonceHorizon > 0`.
25. `MaxIdleStarvationBlocks > 0`.
26. Simulation records are bounded by the same `MaxContractSteps`.
27. Every loop and storage bound appears in metadata or generated descriptors.
28. `MaxSplitTransferLegs >= 2`.
29. No Config binding or storage item exists for `StepBaseFee`, a legacy condition-read fee, stored derived Weight/fee bounds, cache epochs, or generic cache revalidation.
30. `AssetOps` supports source-exhausting Transfer for an admitted terminal custody drain; ordinary source preservation remains owned by Section 3.3 amount resolution.

### 10.2 Semantic Reference Profile

```text
TargetBlockTime = 6 seconds
MaxActiveActors = 10_000                  MaxOwnerSlots = 255
MaxContractSteps = 8                 MaxRetryAttempts = 10
MaxFundingTrackedAssets = 10              MaxOpeningSnapshotEntries = 16
MaxOpeningPredicateResults = 32            MaxPreconditionClauses = 4
MaxPredicatesPerClause = 4                 MaxPredicatesPerStep = 4
MaxCadenceDelayTicks = 631_152_000        MaxWhitelistSize = 16
MaxSplitTransferLegs = 8                  MaxConsecutiveFailures = 10
MaxQueueLength = 10_000                   MaxExecutionDelayBlocks = 52_596_000
MinWindowLength = 100
MaxAutoCloseNonceHorizon = 10_000         MaxIdleStarvationBlocks = 25
MaxSweepBatch = 5
MinUserBalance = 5 * existential deposit
ActorCreationFee = existential deposit
```

`52_596_000` is ten Julian years at six seconds. The reference profile exposes 255 User slots and reserves bitmap bit `255` as the permanently invalid `u8::MAX` sentinel.

### 10.3 Measured Runtime Bindings

`QueuePageSize`, `WakeupPageSize`, `ObservationPageSize`, `MaxQueueEntriesScannedPerBlock`, `MaxExecutionsPerBlock`, `MaxWakeupsPerBlock`, `MaxObservationFanoutPagesPerBlock`, `WakeupWeightLimit`, `ObservationFanoutWeightLimit`, `ActorOnIdleReserve`, base/cleanup bounds, and adapter worst cases are measured runtime outputs.

Generated runtime and Weight descriptors define the values. Count ceilings do not guarantee throughput.

## 11. Conformance

A runtime conforms to this specification iff every requirement holds:

1. Actors runtime metadata exposes only the calls, public types, events, errors, bounds, and runtime APIs defined here.
2. Every reachable control, scheduler, ingress, task, cancellation, and close transition follows the specified state delta, ordering, and rollback boundary.
3. Scheduler admission, sweep, actor-targeting expiry substitution, certified-ingress expiry handling, simulation, and eligibility use one actor-classification implementation and preserve the exact Section 9.2 error projections.
4. User and System fee, mutability, and terminal behavior match Sections 2.2, 2.4, and 4.3.
5. Storage satisfies Section 9.3 and `try_state` verifies its ownership, cardinality, membership, Continuation, terminal, funding, and scheduler invariants.
6. One generated `WeightInfo` supplies every numeric Weight; `WeightToFee` derives every User attempt fee from it.
7. Generated Weight descriptors upper-bound every call, task, worker, ingress path, cleanup path, and admitted maximum composition in RefTime and ProofSize.
8. Certified producer and publisher inventories contain every path that claims Actors ingress or observation authority; paths outside them have no Actors semantic effect.
9. Adapter retry classification is exhaustive and maps unknown failure to Permanent.
10. Simulation is rollback-only and observationally equivalent to one production attempt at the same state and block.
11. Simulation and eligibility project actor classification without independently recomputing terminal or execution-phase predicates, and every classification error follows the exact Section 9.2 mapping.
12. Initial state and every completed migration contain no compatibility alias, dual write, stale derived cache, or permanent generic migration state.
13. Close preserves custody without transfer; exact User-slot and System-locator reattachment derive the same sovereign account; an admitted terminal custody drain can exhaust one named asset, including the fee-native balance net of current attempt fees, and closes atomically on positive success while a zero drain follows the specified non-closing skip path.
14. Canonical input tests prove runtime rejection without sorting or deduplication; task-shape tests prove at most two `AmountResolution` fields and at most two opening surfaces per step; opening-surface admission never truncates; every Section 3.4 step row has identical production, simulation, counter, and event behavior.
15. Fee-collector failure rolls back the complete attempt, projects `FeeCollectionFailed` in simulation, cannot arise solely from missing reserved payer balance, and cannot be downgraded by authored step policy. When a host elects certified FeeSink ingress, that movement never silently degrades to balance-only and notifies exactly once; ledger-only hosts create no trigger consequence.
16. Cutoff tests prove the same-block `MAY`/`MUST NOT` boundary; maximum-plan tests prove bounded paid completion without structural starvation; Active Fee Sink tests prove first-signal placement at maximum canonical occupancy; fee-collection stalls may retry only through later ordinary actor service; no call-level path bypasses `InvariantStalledLiveHead`; deployed-lineage structural repair follows Section 9.4.
17. Cursor-local retry tests distinguish fresh suspension, repeated suspension at the same cursor, and first suspension at a later cursor. Timing tests prove floor-readiness, ceil-activation, a full first cadence period, constant-time missed-period coalescing, no cadence catch-up, Paused actors without ordinary targets, and one typed temporal membership per Actor.

---

_End of specification._
