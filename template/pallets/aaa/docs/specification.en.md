# AAA Specification

- **Scope**: Account Abstraction Actors runtime contract
- **Target**: `pre-1.0.0`
- **Date**: August 2026
- **Status**: Normative

RFC 2119/RFC 8174 key words are normative when uppercase. This document owns behavior and semantic type meaning. Runtime metadata owns exact SCALE encoding; generated storage and Weight descriptors own exact layout and measured numbers; architecture documents own rationale and implementation topology.

---

## 1. Core Contract

- Equal state and block context MUST produce equal behavior.
- Work MUST be O(1) or O(K) under explicit finite bounds and admit complete `Weight(RefTime, ProofSize)` before mutation.
- A plan is a bounded linear `Step[]`: no loops, jumps, nested programs, opaque dispatch, task-authored memory, or authored whole-plan rollback.
- Every admitted path ends in a named completion, skip, failure, suspension, cancellation, close, or state-preserving deferral; unknown capability/failure fails closed.
- `ActorIdentity`, `ActorHot`, `ActorProgram`, `ActorFunding`, and optional `ContinuationState` are the only canonical actor partitions; composites are read-only.
- User and System share one strict FIFO, temporal-readiness layer, cutoff, and service order.
- One generated runtime `WeightInfo` owns every numeric Weight.
- Unless explicitly stated otherwise, arithmetic is checked and amount resolution never clamps.
- Close deletes actor semantics, preserves sovereign balances, and performs no post-close transfer. Locator reuse creates a fresh actor and inherits custody only.
- User creation pays only on committed creation; User executable steps are attempt-priced; AAA has no recurring rent or task-fee refund.

### 1.1 Non-goals

AAA defines no ownership transfer, arbitrary dispatch, loops, same-block actor-graph continuity, priority lane, actual-Weight refund, readiness-bypassing simulation, signal-payload amount resolution, adaptive rent/deposit pricing, or Router route semantics. Uncertified balance movements are not retroactively interpreted as AAA ingress.

Commit terms:

| Term | Contract |
| --- | --- |
| Rejected control transition | The AAA call returns `Err`; AAA state/events, subscriptions, scheduler state, and opening-fee movement equal pre-state. Host fee/nonce accounting is excluded. |
| Provisional task commit | A task layer succeeds inside an admitted scheduler attempt but remains reversible until the enclosing attempt commits. |
| Committed unsuccessful attempt | An admitted attempt durably commits failure or suspension; collected Section 4.3 fees remain charged. It is not a rejected transition. |
| Rolled-back scheduler attempt | The enclosing attempt fails before durability; queue consumption, nonce/attempt, snapshots, fees, tasks, events, and scheduler state equal pre-attempt state. |

A step failure inside an admitted attempt is not a rejected transition. Authored whole-plan rollback is forbidden; rollback of the enclosing scheduler transaction remains required on its own failure.

## 2. Actor Model

### 2.1 Canonical State

```rust
type AaaId = u64;
type OwnerSlot = u8;
type OwnerSlotBitmap = [u8; 32];
type SystemSovereignId = u64;
type QueueTicket = u64;
type ObservationRevision = u64;
type CacheEpoch = u32;

enum AaaType { User, System }
enum InitialLifecycle { Dormant, Active }
enum PauseReason { Manual }
enum ActorClass { User { owner_slot: OwnerSlot }, System { sovereign_id: SystemSovereignId } }
enum SystemSovereignState { Vacant, Occupied(AaaId) }
enum Mutability { Mutable, Immutable }
enum ActiveLifecycle { Active, Paused(PauseReason) }
enum CycleState { Idle, Suspended }

struct ActorIdentity<AccountId, BlockNumber> {
    sovereign_account: AccountId,
    owner: AccountId,
    actor_class: ActorClass,
    mutability: Mutability,
    cycle_nonce: u64,
    last_control_mutation_block: Option<BlockNumber>,
}

struct ActorHot<BlockNumber, Balance> {
    lifecycle: ActiveLifecycle,
    cycle_state: CycleState,
    auto_close_at_cycle_nonce: Option<u64>,
    consecutive_failures: u32,
    pending_signal: bool,
    queue_ticket: Option<QueueTicket>,
    wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
    terminal_at: Option<BlockNumber>,
    cycle_weight_upper: Weight,
    cycle_fee_upper: Balance,
    funding_tracked_count: u32,
    schedule_anchor: BlockNumber,
    last_cycle_block: Option<BlockNumber>,
    cache_epoch: CacheEpoch,
}

struct ActorProgram<Schedule, BlockNumber, ExecutionPlan> {
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow<BlockNumber>>,
    execution_plan: ExecutionPlan,
    completion_policy: CompletionPolicy,
}

struct ActorFunding<Policy, AssetId, Balance> {
    funding_source_policy: Policy,
    funding_accumulated: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
    funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
}

struct ContinuationState<BlockNumber, AssetId, Balance> {
    cursor: u32,
    attempt: u32,
    unsuccessful_attempts_at_cursor: u32,
    last_attempt_block: BlockNumber,
    opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxOpeningSnapshotEntries>,
    funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
    cumulative_outcomes: OutcomeTotals,
}

struct CacheRevalidationState<Cursor> {
    target_epoch: CacheEpoch,
    cursor: Cursor,
    remaining: u32,
}

struct OutcomeTotals {
    executed_steps: u32,
    committed_effectful_tasks: u32,
    skipped_conditions: u32,
    skipped_resolution: u32,
    skipped_funding_unavailable: u32,
    failed_steps: u32,
}

enum CloseReason {
    OwnerInitiated, BalanceExhausted, ConsecutiveFailures, WindowExpired,
    CycleNonceExhausted, FeeBudgetExhausted, AutoCloseNonceReached,
    RetryAttemptsExhausted, ProductiveCycleCompleted,
}
enum CycleResult { Completed, Failed, Cancelled }
enum SuspensionReason { FundingUnavailable, Temporary }
enum CancellationReason {
    Explicit, ExecutionPlanChanged, CompletionPolicyChanged, FundingPolicyChanged,
    ScheduleChanged, Deactivated, Closing(CloseReason), RuntimeUpgrade,
}
enum StepSkippedReason { ConditionsNotMet, ResolutionSkipped, FundingUnavailable }
```

Relations:

- Active iff identity, hot, program, and funding partitions exist; Dormant iff only identity exists.
- `ContinuationState` exists iff `cycle_state == Suspended`.
- An Active actor is executable only when `ActorHot.cache_epoch == CurrentCacheEpoch` and no `CacheRevalidationState` exists.
- `ActorClass` solely determines class; `AaaType` is derived and never stored.
- A composite actor value is read-only and MUST NOT become another write model.
- One runtime `MaxExecutionPlanSteps` applies to both classes; Section 10 owns its constraints and reference value.
- Dormant creation performs no program scan, subscription, funding tracking, readiness, or placement.
- Active admission fits the complete cycle/suffix, probes, and possible cleanup inside Section 5.4 actor-service Weight before fee collection or mutation.

`WakeupPointer` is semantically only the authoritative temporal-membership marker. Runtime metadata/storage descriptors own its encoded fields; no page/index topology follows from this specification.

### 2.2 Class and Mutability

User creation is signed, makes the caller the owner, consumes one owner slot, and pays User AAA fees. System creation requires `SystemOrigin`, names an explicit owner, consumes no User slot, and pays no User AAA fee. Both classes share FIFO order.

Actor-scoped authorization is:

| Actor | Authorized control origin |
| --- | --- |
| User | signed owner |
| System | signed owner or `SystemOrigin` |

This rule includes `manual_trigger`. Unauthorized signed control returns `NotOwner`; an origin that claims governance authority but fails `SystemOrigin` is rejected by the host origin check.

Mutable control may replace schedule/window, execution plan/completion policy, funding policy, lifecycle, Continuation, close target, or Active/Dormant state. User Immutable rejects those controls but MAY invoke an authored Manual readiness signal. System Immutable rejects every actor-scoped control and cannot contain Manual. Internal terminal transitions remain valid.

`RetryLater` is forbidden for every Immutable actor. `2 <= RetryLater.max_attempts <= MaxRetryAttempts`; `max_attempts` counts unsuccessful executions of the cursor including the opening attempt, so it permits at most `max_attempts - 1` retries.

An Immutable actor MAY omit every internally reachable terminal path. Such an actor can remain Active indefinitely, permanently occupying its slot/locator and leaving custody controllable only by its program and mandatory terminal predicates. Admission treats this as an explicit irreversible commitment, not an error.

`SystemOrigin` has no call-level override for System Immutable existence or program state. The sole emergency override is a runtime upgrade governed by the host and executed through Section 9.4; it MUST name the actor disposition, custody consequences, and expected governance latency. AAA itself guarantees no finite social/governance recovery time for this commitment.

### 2.3 Sovereign Accounts and Identifiers

```text
User seed   = Blake2_256(SCALE(AaaPalletId, b"user", owner, owner_slot))
System seed = Blake2_256(SCALE(AaaPalletId, b"system", system_sovereign_id))
account     = AccountId::decode(TrailingZeroInput(seed))
```

The decoder MUST be total and deterministic for every 32-byte seed.

- `OwnerSlotBitmap` has 256 bits; valid slots satisfy `owner_slot < MaxOwnerSlots` under Section 10, and every bit outside that range is zero. The reference profile exposes slots `0..=254`; bit `255` is a permanently invalid sentinel so `u8::MAX` never denotes an owned slot. Default creation scans only bits `< MaxOwnerSlots`, masks every higher bit, and selects the lowest free valid bit; exact-slot creation requires a valid free bit; close clears it.
- Fresh System creation uses `system_sovereign_id = aaa_id`, records `Occupied(aaa_id)`, and consumes lifetime locator capacity. Close marks it `Vacant`; deactivation leaves it occupied; reuse requires a vacant locator and a fresh actor id.
- `SystemSovereignCount` includes vacant locators; reuse does not change it. `SovereignIndex` covers current Active/Dormant ownership only.
- `SovereignAccountPolicy` rejects reserved accounts. Live collision, reserved collision, unknown/occupied locator, and capacity exhaustion remain distinct errors.
- `NextAaaId` and queue tickets checked-increment and never repeat.
- Reattachment inherits adapter-exposed custody only; owner, program, nonce, funding, Continuation, clocks, readiness, and guarantees restart.
- Custody derivation survives host account-provider removal. Every AAA-declared ordinary debit obeys the protected minimum in Section 3.3; a successful User fee-native debit additionally preserves `MinUserBalance`, and adapters MUST NOT add hidden debits. External host actions MAY still reap or dust an account; deterministic reattachment does not recreate value removed by host policy.

### 2.4 Lifecycle, Failure Streak, and Terminal Order

Checks precede the requested effect in this order. The order applies only among predicates evaluated by the owning transition defined below:

| Condition | Close reason |
| --- | --- |
| `current_block > window.end` | `WindowExpired` |
| User fee-native balance `< MinUserBalance` before opening | `BalanceExhausted` |
| User fee budget `< attempt_fee_envelope(plan, cursor, User).total` | `FeeBudgetExhausted` |
| `cycle_nonce == u64::MAX` before Active installation/opening | `CycleNonceExhausted` |
| cursor-local unsuccessful executions reach `max_attempts` | `RetryAttemptsExhausted` |
| `consecutive_failures` reaches `MaxConsecutiveFailures` | `ConsecutiveFailures` |
| completed productive cycle under `CloseAfterProductiveCycle` | `ProductiveCycleCompleted` |
| completed cycle reaches auto-close target | `AutoCloseNonceReached` |

System ignores User balance/fee checks. Productive close precedes auto-close. Windows are inclusive; terminal readiness occurs at representable `end + 1`.

Terminal predicates have one owner:

- `WindowExpired` is checked by actor-targeting control, address-event notification, scheduler admission, and sweep.
- `BalanceExhausted` and `FeeBudgetExhausted` are User attempt-opening or sweep predicates.
- `CycleNonceExhausted` is checked before Active installation, fresh-cycle opening, or sweep.
- Retry/global-failure and completion-driven reasons are applied only by attempt finalization.

Address-event notification therefore performs inline terminal close only for `WindowExpired`; receipt of value alone does not infer another close reason.

An admitted attempt is unsuccessful exactly when it commits `Suspended` or terminal `Failed`.

- Any `SuspendCurrent` outcome — `FundingUnavailable` or Temporary `TaskFailure` — increments `unsuccessful_attempts_at_cursor` and `consecutive_failures` exactly once.
- A terminal `Failed` attempt increments `consecutive_failures` exactly once.
- `Completed` resets `consecutive_failures` to zero before rearm and completion-driven close evaluation.
- Deferral, cancellation, pause/resume, exact no-op, an advancing `FundingUnavailable` skip, and an individual `ContinueNextStep` failure do not independently change either counter.
- Fresh Active installation and semantic execution-plan or completion-policy replacement reset the global streak; schedule and funding-policy replacement preserve it.

For a suspension at cursor `c`:

```text
next_local =
    1 if c differs from the previously suspended cursor;
    checked_add(unsuccessful_attempts_at_cursor, 1) otherwise

next_global =
    checked_add(consecutive_failures, 1)

backoff_index =
    checked_sub(next_local, 1)

backoff(backoff_index 0,1,2,>=3) =
    1,2,4,8 blocks

if next_local >= max_attempts:
    finalize Failed; close RetryAttemptsExhausted
else if next_global >= MaxConsecutiveFailures:
    finalize Failed; close ConsecutiveFailures
else:
    persist suspension with next_local and next_global
```

Local precedence wins when both bounds are reached by the same attempt. `ContinuationState.attempt` counts continuation executions across the cycle for event identity; it does not select backoff. Cursor-local counting restarts when a later cursor first suspends. A non-retry terminal `Failed` attempt applies only `next_global`; threshold close follows `CycleSummary(Failed)`.

Fresh Active creation and activation install one Active epoch:

| Surface | Installed value |
| --- | --- |
| identity nonce | `0` for fresh creation; preserved Dormant nonce for activation |
| lifecycle/cycle | `Active / Idle`; no Continuation |
| schedule clocks | `schedule_anchor = max(now, window.start)` when a future window exists, otherwise `now`; `last_cycle_block = None`; `terminal_at = end + 1` or `None` |
| readiness | `pending_signal = false`; one canonical FIFO/temporal path derived from the schedule, or none when no readiness is due |
| failure/economics | `consecutive_failures = 0`; empty accumulator; tracked set and cycle Weight/fee caches recomputed from the program |
| cache | `cache_epoch = CurrentCacheEpoch` |
| subscriptions | exact feeds derived from trigger sources |
| persistent control clock | identity `last_control_mutation_block = now` |

Activation does not increment `cycle_nonce`. Every later fresh cycle computes:

```text
next_cycle_nonce = checked_add(stored_cycle_nonce, 1)
```

and `CycleStarted.cycle_nonce == next_cycle_nonce`. Retries reuse the opened nonce. Therefore a Dormant identity carrying nonce `n` remains at `n` on activation and opens its next cycle at `n + 1`.

The following transition deltas are canonical:

| Transition | Preserved / changed |
| --- | --- |
| pause/resume | preserve program, funding, Continuation, failure streak, latch, schedule clocks, terminal target, and cache epoch; atomically remove/reconstruct ordinary placement |
| semantic schedule/window replacement | cancel Continuation, diff subscriptions, reset `schedule_anchor` and `terminal_at`, preserve funding accumulator, latch, failure streak, and cache epoch, then reconstruct placement |
| semantic plan/completion replacement | cancel Continuation, recompute tracked assets and current caches, delete accumulator entries no longer tracked, reset failure streak, preserve schedule/latch, set current cache epoch, then reconstruct placement |
| semantic funding-policy replacement | cancel Continuation, preserve accumulator, tracked set, schedule/latch, failure streak, and cache epoch, then reconstruct placement when required |
| auto-close target change | change only the target; do not cancel Continuation or consume the control-mutation limit |
| deactivation | cancel Continuation and delete the complete Active epoch while preserving identity, locator, nonce, persistent control clock, and balances |

Every creation initializes the persistent identity control clock. Thereafter `activate_aaa`, `deactivate_aaa`, `pause_aaa`, `resume_aaa`, semantic `update_schedule`, semantic `update_execution_plan`, semantic `update_funding_source_policy`, and `cancel_continuation` are limited to one committed call per actor per block. Activation checks and updates the identity clock before installing Active state; deactivation updates it before deleting Active state. Exact no-op returns before checking or updating the clock. `manual_trigger`, auto-close target calls, explicit close, sweep, global controls, ingress, and internal scheduler/cleanup transitions are exempt. Rejection uses `ControlMutationRateLimited`.

Close performs bounded deletion/index repair, preserves balances, and releases the User slot or marks the System locator vacant. `Persistent` remains Active after completion; `CloseAfterProductiveCycle` requires `committed_effectful_tasks > 0`.

### 2.5 Continuation, Funding Delta, and Cycle Accounting

Explicit Continuation cancellation requires Mutable control and `cycle_state == Suspended`. Semantic plan/completion, funding-policy, or schedule/window replacement; deactivation; close; expiry; or incompatible upgrade cancels before changed meaning applies. Exact no-op and auto-close-target changes do not cancel. Pause/resume and breaker preserve Continuation. Cancellation performs no compensation, funding restoration, prefix rollback, or balance movement and emits `CycleCancelled`, then `CycleSummary(Cancelled)`.

If close encounters a Continuation, cancellation uses `Closing(reason)` and precedes `AaaClosed(reason)`. A pure close means no Continuation exists and emits only `AaaClosed`.

Funding authority is independent from trigger matching:

| Policy | Authoritative funding acceptance |
| --- | --- |
| `OwnerOnly` | `provenance == Signed` and concrete `source == owner` |
| `SignedAllowlist` | `provenance == Signed` and concrete `source` is allowlisted |
| `RuntimePolicy` | `FundingAuthority::permits` accepts the exact source/provenance pair; the all-`None` pair MUST be denied |
| `AnyVerifiedIngress` | `source.is_some()` or `provenance.is_some()`; certification alone does not make the all-`None` pair acceptable |

Program admission derives the bounded `funding_tracked_assets`, including `StakingOps::share_asset(position)` for `PercentageOfLastFunding` used by Unstake. `AnyVerifiedIngress` permits third parties to shape the next funding basis only by delivering real accepted value; it grants no withdrawal authority and every debit remains bounded by current capacity. A credit in an untracked asset, or a tracked credit rejected by funding policy, is balance-only: it does not mutate `funding_accumulated` and emits no `FundingAccumulated`; trigger matching may still latch readiness. A tracked accepted positive credit checked-adds to the accumulator. Bound or arithmetic overflow returns `FundingAccumulatorOverflow` and rolls back the producer transaction.

When semantic plan/completion replacement removes an asset from the tracked set, deleting its accumulator entry changes only future `PercentageOfLastFunding` resolution. It does not move, burn, reserve, lock, or otherwise alter the corresponding account balance.

`PercentageOfLastFunding` means accepted tracked funding since the previous logical cycle opening in the current Active epoch, or since Active installation for the first cycle. Section 4.2 owns the exact snapshot, clear, latch, nonce, and event order.

On suspension, the persisted funding snapshot is the exact projection of the pre-clear funding snapshot onto assets referenced by `PercentageOfLastFunding` in `execution_plan[cursor..]`, including the unresolved cursor step and mapped Unstake share assets. Entries outside that suffix cannot be read because the cursor never decreases; dropping them is semantics-preserving and does not restore them to the next accumulator. Projection retains every present suffix-referenced entry with its exact opening value and synthesizes no missing key. Absence means no accepted funding existed at opening and continues to resolve as `FundingUnavailable`.

Later funding belongs to the next cycle. Completion, failure, cancellation, pause, and breaker neither restore the consumed snapshot nor alter the next accumulator. Deactivation/close delete accumulation; `funding_tracked_count` equals the tracked set.

A cycle completes at plan end or successful `StopCycle`; accepted `ContinueNextStep` failures may coexist with `Completed`. `AbortCycle`, Permanent `RetryLater`, exhausted bounds, and cancellation do not complete. `cycle_nonce` increments once per cycle; retries reuse it and increment `attempt`; `last_cycle_block` records opening and `last_attempt_block` the latest admitted attempt. Deferral changes no cycle state or event stream. Every task commit remains provisional through final placement/cleanup. A late capacity, namespace, or invariant failure in the enclosing scheduler attempt rolls back the complete attempt, including task effects and fees. Signals during an open cycle affect only possible future readiness.

## 3. Program Model

### 3.1 Public Types

```rust
enum ProgramInput<S, B, P, F> { Dormant, Active(ActiveProgramInput<S, B, P, F>) }
struct ActiveProgramInput<S, B, P, F> {
    schedule: S,
    schedule_window: Option<ScheduleWindow<B>>,
    execution_plan: P,
    completion_policy: CompletionPolicy,
    funding_source_policy: F,
    auto_close_at_cycle_nonce: Option<u64>,
}

struct Schedule<Sources> { trigger: TriggerPolicy<Sources>, cooldown_blocks: u32 }
struct ScheduleWindow<BlockNumber> { start: BlockNumber, end: BlockNumber }
enum TriggerPolicy<Sources> { Immediate { sources: Sources }, Cadenced { every_blocks: u32, mode: CadenceMode<Sources> } }
enum CadenceMode<Sources> { Always, WhenSignalled(Sources) }
enum TriggerSource<AccountId, AssetId, FeedId> {
    Manual,
    OnAddressEvent { source_filter: SourceFilter<AccountId>, asset_filter: AssetFilter<AssetId> },
    OnObservationChange { feed: FeedId },
}
enum SourceFilter<AccountId> { Any, OwnerOnly, Whitelist(BoundedVec<AccountId, MaxWhitelistSize>) }
enum AssetFilter<AssetId> { Any, Whitelist(BoundedVec<AssetId, MaxWhitelistSize>) }
enum FundingSourcePolicy<AccountId> { OwnerOnly, SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>), RuntimePolicy, AnyVerifiedIngress }
enum FundingProvenance { Signed, InternalProtocol, Xcm }
struct AddressEvent<AccountId, AssetId, Balance> {
    destination: AccountId,
    source: Option<AccountId>,
    asset: AssetId,
    amount: Balance,
    provenance: Option<FundingProvenance>,
}

enum AmountResolution<Balance> { Fixed(Balance), PercentageOfCurrent(Perbill), PercentageAtOpening(Perbill), PercentageOfLastFunding(Perbill), AllAvailable }
enum OpeningSurface<AssetId> { PreservableAsset(AssetId), TargetAsset(AssetId), StakingShares(AssetId) }
enum InputLimit<Balance> { LiveQuote, Absolute(Balance) }

struct Step<Condition, Task> { conditions: ConditionSet<Condition, MaxConditionsPerStep>, task: Task, on_error: StepErrorPolicy }
enum ConditionSet<C, Max> { Always, All(BoundedVec<C, Max>), Any(BoundedVec<C, Max>) }
enum Condition<AssetId, Balance, BlockNumber, FeedId> {
    BalanceAbove { asset: AssetId, threshold: Balance }, BalanceBelow { asset: AssetId, threshold: Balance },
    BalanceEquals { asset: AssetId, threshold: Balance }, BalanceNotEquals { asset: AssetId, threshold: Balance },
    BlockNumberAbove { threshold: BlockNumber }, BlockNumberBelow { threshold: BlockNumber },
    ObservationAbove { feed: FeedId, threshold: u128, max_age_blocks: u32 },
    ObservationBelow { feed: FeedId, threshold: u128, max_age_blocks: u32 },
    ObservationEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
    ObservationNotEquals { feed: FeedId, threshold: u128, max_age_blocks: u32 },
}

struct SplitLeg<AccountId> { to: AccountId, share: Perbill }
enum Task<AccountId, AssetId, Balance> {
    Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
    SplitTransfer { asset: AssetId, amount: AmountResolution<Balance>, legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs> },
    SwapIn { asset_in: AssetId, amount_in: AmountResolution<Balance>, asset_out: AssetId, slippage_tolerance: Perbill },
    SwapOut { asset_out: AssetId, amount_out: AmountResolution<Balance>, asset_in: AssetId, input_limit: InputLimit<Balance>, slippage_tolerance: Perbill },
    AddLiquidity { asset_a: AssetId, asset_b: AssetId, amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance>, min_lp_out: Balance },
    RemoveLiquidity { lp_asset: AssetId, asset_a: AssetId, asset_b: AssetId, lp_amount: AmountResolution<Balance>, min_amount_a: Balance, min_amount_b: Balance },
    Burn { asset: AssetId, amount: AmountResolution<Balance> }, Mint { asset: AssetId, amount: AmountResolution<Balance> },
    Stake { asset: AssetId, amount: AmountResolution<Balance> },
    DonateLiquidity { asset_a: AssetId, asset_b: AssetId, max_amount_a: AmountResolution<Balance>, max_ratio_error: Perbill },
    Unstake { asset: AssetId, shares: AmountResolution<Balance> }, StopCycle,
}

enum StepErrorPolicy { AbortCycle, ContinueNextStep, RetryLater { max_attempts: u32 } }
enum CompletionPolicy { Persistent, CloseAfterProductiveCycle }
```

Canonical source sets, filters, and allowlists are nonempty where required, duplicate-free, and strictly ordered by canonical SCALE bytes. Admission rejects repeated Manual atoms, repeated address-filter pairs, repeated feeds, empty whitelists, and duplicate members. Longer workflows compose actors asynchronously and gain no same-block or one-event-one-cycle guarantee.

### 3.2 Triggers and Timing

| Policy | Readiness |
| --- | --- |
| `Immediate` | requires `pending_signal` |
| `Cadenced::WhenSignalled` | latches immediately; opens no earlier than cadence |
| `Cadenced::Always` | opens on cadence without a latch; rearms after terminal cycle while Active |

Sources compose as OR and are fully evaluated without short-circuit. A trigger changes readiness only. `pending_signal` is the sole latch; matches only set `false -> true`; Section 4.2 defines its opening reset. An AddressEvent without concrete source matches only `SourceFilter::Any`.

Fresh Active installation does not manufacture a signal. Consequently, `Immediate` and `Cadenced::WhenSignalled` remain unready until one configured source matches. `every_blocks` defines earliest eligibility, not a guarantee of service or one execution per cadence period.

```text
cooldown_anchor = last_cycle_block.or(schedule_anchor)

cooldown_eligible_at =
    schedule_anchor
        if cycle_nonce == 0 && last_cycle_block is None;
    checked_add(cooldown_anchor, cooldown_blocks)
        otherwise

phase_window = min(every_blocks / 4, MaxTimerJitterBlocks)

cadence_phase =
    0 if phase_window == 0;
    u64_le(Blake2_256(SCALE(aaa_id))[0..8]) % phase_window
        otherwise

cadence_origin =
    checked_add(schedule_anchor, cadence_phase)

cadence_at_or_after(lower) =
    least checked(cadence_origin + k * every_blocks), k >= 0,
    that is >= lower

window_floor =
    schedule_window.start when a window exists;
    BlockNumber::zero() otherwise

temporal_eligible_at =
    max(cooldown_eligible_at, window_floor)
        for Immediate;
    cadence_at_or_after(
        max(
            cooldown_eligible_at,
            window_floor,
            checked_add(last_cycle_block, 1) when last_cycle_block exists
        )
    )
        for Cadenced
```

For Immediate, the cadence term is absent. For signal-driven policies, temporal eligibility does not imply readiness; the latch must also exist.

Retry timing is owned by Section 2.4:

```text
retry eligibility =
    checked_add(
        last_attempt_block,
        max(cooldown_blocks, backoff(unsuccessful_attempts_at_cursor - 1))
    )
```

Missed cadence points coalesce; they do not create catch-up cycles. Jitter shifts phase once and does not change the cadence period. Every composed delay fits `MaxExecutionDelayBlocks`.

For window installation:

```text
start_delay = start.saturating_sub(now)
first_temporal_eligible =
    temporal_eligible_at under the prospective Active state
```

Validation requires `end > start`, representable `end + 1`, inclusive length `>= MinWindowLength`, `now <= end`, `start_delay <= MaxExecutionDelayBlocks`, and `first_temporal_eligible <= end`. For `Immediate` and `Cadenced::WhenSignalled`, the final predicate proves only that a temporal opportunity exists; no future signal is assumed.

### 3.3 Conditions and Amounts

`Always` is true with zero atoms. `All/Any` contain `1..=MaxConditionsPerStep` atomic conditions. Production and simulation read every atom without short-circuit; any atom error yields Permanent failure. Observation `Stale`, `Uninitialized`, or `Unavailable` is false; invalid `Fresh` is Permanent; `max_age_blocks > 0`. Conditions are pure.

The subject of every `Balance*` condition is the actor's sovereign account; conditions cannot read a third-party account. They always use the ordinary `AssetOps::balance` surface. For `FeeNativeAssetId`, they subtract the current transient fee reservation but not the protected minimum. They never redirect to `StakingOps::share_balance`. A balance condition may therefore pass while resolution returns `FundingUnavailable` because conditions observe and resolution authorizes spending.

Percentages use widened floor division under Section 1. `PercentageOfCurrent` and `PercentageAtOpening` use the same policy-specific surface and differ only in read time. `PercentageAtOpening` never reads a signal payload or AddressEvent amount.

| Policy | Tasks | Current/opening base and capacity |
| --- | --- | --- |
| preserve source | Transfer, SplitTransfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | `AllAvailable` and `PercentageOfCurrent` use `preservable_balance`; `PercentageAtOpening` reads `OpeningSurface::PreservableAsset`; fixed/opening/funding values must fit current preservable capacity |
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
```

`reserved_fee_remaining` is zero for System attempts.

A field yields:

- `Resolved(value)` for a positive valid value;
- `Skipped` for a valid dynamic zero, including a zero current or opening basis;
- `FundingUnavailable` for an absent/zero `PercentageOfLastFunding` basis, a positive resolved debit that exceeds current source/share capacity, or a required auxiliary debit cap of zero.

Every admitted `PercentageAtOpening` surface exists in `opening_snapshot`, including zero-valued surfaces. A missing key is `SnapshotUnavailable` and indicates invariant failure.

Multi-surface aggregation is the sole precedence rule:

```text
any FundingUnavailable -> FundingUnavailable
else any Skipped        -> Skipped
else                    -> Executable(all resolved fields)
```

Admission rejects zero fixed/percentage/absolute bounds, `AllAvailable` on Mint/SwapOut, identical pairs, `Transfer.to == sovereign_account`, any split recipient equal to the sovereign account, duplicate split recipients, mismatched LP pair, unsupported/class-forbidden modes, and zero required liquidity minima. Self-recipient rejection uses `SelfTransferNotAllowed`.

`AllAvailable` preserves the current protected minimum but reserves no budget for a later cycle; a later User opening may therefore close with `FeeBudgetExhausted`.

### 3.4 Tasks and Canonical Step Control

Task variants in Section 3.1 own their declared asset/recipient fields. Mint is System-only. Resolved task amounts have these exact meanings:

| Task | Debit / output contract |
| --- | --- |
| Transfer, Burn | debit exactly the resolved amount |
| Stake | stake exactly the resolved amount |
| Unstake | unstake exactly the resolved shares; `AllAvailable` first resolves to the full current share balance |
| SwapIn | debit exactly the resolved input; `DexOps` owns the executable quote and output protection |
| SwapOut | credit exactly the resolved output; AAA supplies one finite input-cap ceiling and `DexOps` owns the executable quote/tolerance cap |
| AddLiquidity | resolved `amount_a` and `amount_b` are debit caps; return exact positive used amounts and LP output meeting `min_lp_out` |
| RemoveLiquidity | debit exactly the resolved `lp_amount`; `min_amount_a` and `min_amount_b` are output floors |
| DonateLiquidity | `max_amount_a` and derived `max_amount_b` are debit caps; return exact positive used amounts within both caps |

Ordered LP identity is validated at admission and rechecked at execution. `RemoveLiquidity` MUST NOT perform a smaller partial LP debit than the resolved `lp_amount`.

`DonateLiquidity.max_amount_a` resolves under preserve-source policy. At step preparation:

```text
max_amount_b =
    preservable_balance(actor, asset_b, reserved_fee_remaining)
```

`max_amount_a` retains its Section 3.3 resolution outcome. A derived `max_amount_b == 0` contributes `FundingUnavailable`; the aggregate then follows Section 3.3. The adapter receives both finite caps and `max_ratio_error`; it MUST NOT invent a larger cap. Dynamic pool ratio/liquidity/cap movement is Temporary; malformed pair or configuration is Permanent. `LiquidityDonated` records the exact derived caps and actual used amounts.

The canonical step-control matrix is:

| Outcome | `ContinueNextStep` | `AbortCycle` | Mutable `RetryLater` |
| --- | --- | --- | --- |
| Conditions false | skip `ConditionsNotMet`; advance | same | same |
| Resolution `Skipped` | skip `ResolutionSkipped`; advance | same | same |
| `FundingUnavailable` | skip `FundingUnavailable`; advance | same | suspend |
| Temporary failure | record failure; advance | Failed terminal | suspend |
| Permanent failure | record failure; advance | Failed terminal | Failed terminal |
| Success | advance; `StopCycle` completes | same | same |

A `FundingUnavailable` or Temporary outcome under Mutable `RetryLater` enters the identical suspension-accounting path in Section 2.4. `RetryLater` on Immutable is rejected at admission. The cursor only remains, increments by one, or terminates; only `StopCycle` selects early successful terminal. Successful `StopCycle` increments `executed_steps`, not `committed_effectful_tasks`, emits `CycleStopped`, and finalizes `Completed`.

For SwapOut, AAA derives only the current authored/custody ceiling:

```text
capacity_cap =
    preservable_balance(actor, asset_in, reserved_fee_remaining)

authored_cap =
    capacity_cap
        for InputLimit::LiveQuote;
    min(capacity_cap, absolute)
        for InputLimit::Absolute(absolute)
```

`authored_cap == 0` is `FundingUnavailable`. No AAA-side quote call exists.

Inside the single `DexOps` task layer, the adapter obtains one current executable quote before mutation and derives:

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

The adapter MUST return Temporary failure before mutation when the quote is unavailable/zero, `effective_max_in < quote_required_in`, or execution cannot satisfy the exact output within `effective_max_in`. `InputLimit::Absolute` is therefore a cap, not an admission gate.

For SwapIn, the adapter obtains one current executable quote before mutation and requires:

```text
actual_out > 0
actual_out >= floor((1 - tolerance) * quote_output)
```

`slippage_tolerance == Perbill::one()` is allowed but does not waive `actual_out > 0` or any finite SwapOut cap. Exact-input quotes are never inverted across balance width.

The adapter's quote, tolerance arithmetic, System guard, Router call, actual-output validation, paired ingress, rollback, and success event are one task Weight class and one task transaction. AAA exposes no competing quote surface.

The package derives one exhaustive nonnumeric semantic classification for every Task, Condition, AmountResolution, and StepErrorPolicy. Adding a variant fails compilation until ownership, effects, class, control, bounded algorithm, Weight selector, amount dependencies, and failure handling are classified.

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

### 4.1 Attempt Sequence

1. Check lifecycle, readiness, time, breaker, terminal state, and class.
2. Admit complete Weight; for User, admit balance and fee envelope.
3. Load Continuation inputs or prepare fresh trigger/funding snapshots from pre-opening state.
4. For a fresh cycle, atomically increment nonce, consume the signal latch and funding accumulator, update opening clocks, and emit `CycleStarted`; for a retry, atomically increment attempt and emit `CycleContinued`.
5. For each suffix step: evaluate, resolve, collect one selected User fee, execute one nested task layer when executable, and apply Section 3.4.
6. Emit suspension or terminal summary, update failure streak, and apply close policy.
7. Install at most one future scheduler path, close, or install none when Section 2.4 finds no readiness due.
8. Commit the scheduler-attempt transaction.

New cycles start at cursor `0`; retries at the Continuation cursor. `executions(aaa_id, block) <= 1`.

### 4.2 Snapshots and Fresh Opening

Fresh-cycle read-only preparation occurs after fee reservation and before `CycleStarted`.

```text
opening_snapshot =
    every unique policy-specific OpeningSurface referenced by
    PercentageAtOpening in execution_plan[0..]

proposed_funding_snapshot =
    funding_accumulated as observed before opening
```

Opening surfaces are:

```text
PreservableAsset(asset) -> preservable_balance(actor, asset, reservation)
TargetAsset(asset)      -> spendable_balance(actor, asset, reservation)
StakingShares(asset)    -> StakingOps::share_balance(actor, asset)
```

The opening snapshot is independent of trigger kind, signal payload, and `funding_tracked_assets`. Every admitted surface is present even when its value is zero. Program admission requires every share mapping to exist. The snapshot contains no sender, event amount, or event list.

After every fallible opening check succeeds, one scheduler-attempt transaction atomically performs:

```text
next_cycle_nonce = checked_add(stored_cycle_nonce, 1)
stored_cycle_nonce = next_cycle_nonce
pending_signal = false
last_cycle_block = now
funding_accumulated = empty
emit CycleStarted { cycle_nonce: next_cycle_nonce }
```

The first step reads the exact opening snapshot and the pre-clear `proposed_funding_snapshot`. A failed pre-opening admission consumes neither latch nor accumulator and emits no `CycleStarted`. A signal accepted after opening may latch the next cycle but never changes the open snapshots.

Retry performs no new snapshot capture. Suspension persists:

```text
opening_snapshot restricted to OpeningSurface keys referenced by
    PercentageAtOpening in execution_plan[cursor..]

funding_snapshot restricted to entries present in proposed_funding_snapshot and
    referenced by PercentageOfLastFunding in execution_plan[cursor..]
```

The unresolved cursor step is part of the suffix. Projection cannot change future resolution because the cursor never decreases. An absent funding entry denotes zero accepted funding at opening; a missing admitted opening key is `SnapshotUnavailable`.

### 4.3 User Fees

```text
fee_native_balance =
    AssetOps::balance(actor, FeeNativeAssetId)

available_fee_budget =
    fee_native_balance.saturating_sub(MinUserBalance)

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

attempt_fee_envelope(_, _, System).total = 0
```

`step_evaluation_weight_upper` includes complete condition evaluation, opening/current/funding amount preparation, fee collection, and the largest non-task step event reachable before task dispatch. It is selected from the same generated `WeightInfo` as execution. A conforming `WeightToFee` maps every admitted User evaluation and task upper bound to a positive fee.

One `attempt_fee_envelope(plan, cursor, class)` owns admission, reservation, execution, simulation, benchmarks, and generated client vectors.

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

The full step envelope leaves the reservation exactly once regardless of condition, resolution, failure class, or charged amount. The decrement occurs after that step's conditions/resolution and before any later step's conditions/resolution. If the cycle terminates or suspends at step `i`, no later resolution occurs. Fee collection occurs at most once per visited step.

Skip and pre-task resolution failure charge evaluation only. A task attempt charges the combined upper bound, not measured actual Weight. Adapter failure retains that combined fee only if the enclosing scheduler attempt commits. Retry creates a new suffix envelope and charges again. Fee-collector failure dispatches no task, yields Permanent failure, and follows the step policy. Unused reservation is not charged; durable fees are never refunded.

User creation charges `AaaCreationFee` inside its control transaction only; inability to collect that opening fee returns `InsufficientFee`, and rejected creation restores every fee effect. System creation and terminal cleanup charge no AAA fee.

### 4.4 Atomicity

- **Task layer**: adapter movement, paired ingress, and success event provisionally commit or roll back together. Each successful task layer is a nested provisional commit and becomes durable only when the enclosing scheduler-attempt transaction commits. Task failure preserves earlier provisional steps and the selected fee within the enclosing attempt.
- **Control transaction**: identity/locator, lifecycle, program/funding, Continuation, subscriptions, readiness, membership, counters, opening fee, events, and required placement commit together. `Err` is a rejected transition. Exact no-op mutates/emits nothing; expiry substitutes atomic close.
- **Scheduler-attempt transaction**: head consumption, opening/retry, nested task layers, finalization, and future placement/close commit together. Final placement may fail through ordinary capacity/namespace exhaustion or defensive invariant validation; either failure rolls back the complete attempt, including provisional tasks and fees. Authored task failure follows Section 3.4 instead.
- **Terminal cleanup**: preflight ownership/index consequences, then delete/repair without task, fee, balance movement, retry, or requeue.

AAA uses explicit storage transactions; FRAME dispatch failure alone is insufficient. Fault injection covers every fallible ownership, namespace, placement, and producer boundary.

## 5. Scheduler and Reactive Ingress

### 5.1 FIFO and Temporal Readiness

One logical FIFO owns ordinary readiness. Each actor has at most one live ticket; stale physical entries have no authority. After bounded temporal and observation work, `on_idle` snapshots `cutoff = NextQueueTicket`; only older tickets execute.

Before cutoff snapshot, readiness for `now + 1` uses exact temporal readiness. After snapshot it MAY use a ticket, necessarily `>= cutoff`, which cannot execute in the current pass. Readiness `<= now` uses FIFO; later readiness uses an exact target. Queue saturation preserves readiness through an exact next-block target.

Each actor has at most one ordinary temporal requirement and one terminal requirement. They MAY coexist semantically. `wakeup_pointer` authorizes ordinary temporal membership; `terminal_at` authorizes terminal membership and supplies the terminal-removal key together with `aaa_id`. Ordinary temporal membership and a live ticket are exclusive; after ordinary readiness materializes into a ticket, a later terminal requirement remains represented and MAY coexist with that ticket.

The earlier due requirement determines the next temporal service point. If ordinary and terminal requirements are due in the same block, terminal eligibility is evaluated first. A satisfied terminal predicate closes the actor and discards ordinary readiness; otherwise ordinary readiness materializes normally. Temporal insert/replace/remove, earliest-due discovery, and materialization are bounded and scan neither empty block ranges nor all actors. Materialization and ownership removal are atomic.

After bounded stale cleanup, actor service admits the live head or stops; it never scans behind, retickets, or demotes it.

| Discovery | Meaning |
| --- | --- |
| `Empty` | no live eligible pre-cutoff entry remains after bounded stale cleanup; post-cutoff work is outside this pass |
| `Head` | canonical live pre-cutoff head is known |
| `BlockedByWeight` | live head work exists but the complete next discovery/service unit does not fit the current meter |
| `BlockedOther` | scan/attempt ceiling, same-block at-most-once guard, or defensive invariant validation prevents safe progress |

Blocked outcomes preserve order and stop service; only `BlockedByWeight` drives starvation. Breaker and cache revalidation are handled before discovery. Weight/scan deferral changes no state or event. Liveness assumes recurring conforming budget, finite stale churn, eventual capacity, and no external lifecycle blockage.

### 5.2 Observation Delivery

Subscriptions derive only from `OnObservationChange`. Runtime maintains bounded exact actor/feed and reverse ownership; one fanout unit addresses at most `ObservationPageSize` positions. Physical topology is implementation-owned.

Total subscriptions are bounded by `MaxActiveActors * MaxTriggerSources`; distinct subscribed feeds cannot exceed total subscriptions; dirty obligations cannot exceed distinct subscribed feeds. No separate unbounded feed registry exists.

Creation/activation installs, schedule replacement diffs, and deactivation/close removes subscriptions inside the owning control transaction. Installing the first subscriber infers no historical revision and creates no dirty obligation. Publication while a feed has no subscribers allocates no baseline or dirty state. The first later accepted changed publication with subscribers sets the baseline and creates one dirty obligation. Removing the final subscriber deletes both.

The generated certified-publisher inventory is the sole observation-publication authority. Each certified publisher owns the monotonically increasing revision sequence for its feeds and calls `ObservationChangeIngress::note_observation_changed(feed, revision)`. AAA validates but does not synthesize publisher revisions.

Changed publication atomically maintains one highest accepted revision and one pending obligation per subscribed feed. Revision `0` or regression fails; equality is no-op; greater revision updates. Publication is O(1) and does not inspect subscriber groups, mutate actors, enqueue, evaluate, or execute. A publication path outside the certified publisher inventory has no AAA observation effect.

Fanout runs before cutoff. One admitted unit reserves complete Weight, visits one bounded group, latches live actors, and ensures future placement before advancing durable progress. A fanout pass snapshots one revision. Newer revisions update only the latest revision and do not reset the current pass; after the current pass reaches its end, a newer revision starts the next pass. Under recurring budget, eventual placement capacity, and finite subscription churn, each snapshotted pass completes even under continuing publications, although the feed may remain dirty indefinitely. Completion means subscriber groups were visited, not actors executed.

### 5.3 Address-Event Ingress

Only a **certified producer** creates AAA AddressEvent semantics. The generated certified-producer inventory is the sole producer authority: it names every runtime movement path that claims AAA ingress and records its provenance, source availability, atomic rollback owner, preflight/notify integration, retry mapping, and Weight evidence.

AAA `Transfer`, every SplitTransfer leg, and `Mint` are certified producers whenever their destination resolves to an AAA sovereign account. Section 6.2 owns the typed ingress interface.

A certified movement uses one outer transaction:

```text
AddressEventIngress::preflight(event)
-> value movement
-> AddressEventIngress::notify(event)
```

Preflight is read-only and covers lifecycle, funding, trigger, and required placement. Notify executes exactly once after movement. Any failure restores movement and every AAA effect.

The contract intentionally permits a certified third-party movement to an Active actor to fail when its complete AAA consequence cannot commit. Recoverable queue/wakeup capacity or placement unavailability is Temporary. Monotonic ticket/index exhaustion, topology corruption, invalid provenance, and invariant failure are Permanent. An AAA task preserves this classification through `TaskFailure`; a non-AAA producer maps the same failure to its outer dispatch error.

A balance movement not named in the certified inventory is **balance-only**: it cannot latch readiness, update `funding_accumulated`, or emit `FundingAccumulated`, even when its destination is an AAA sovereign account. It is not rejected by AAA scheduler pressure. No event scan, balance-diff scan, inbox, or implicit producer discovery is permitted.

- Absent or Dormant destination is balance-only.
- Zero or self/no-op movement creates no AAA ingress.
- Terminal handling follows Section 2.4 before funding/readiness processing. If notification closes the actor, the credited value remains on the sovereign account.
- Funding acceptance follows Section 2.5. Untracked or policy-rejected credit is balance-only for funding but MAY independently match an AddressEvent trigger.
- Concrete source and typed provenance remain independent. Equal certified movements count separately for funding while readiness coalesces.
- Fee movement belongs only to a certified `FeeCollector` path and cannot notify twice.

### 5.4 Hooks and Reserved Actor Service

`on_initialize` performs no AAA work and returns zero AAA Weight.

`AaaOnIdleReserve` is an embedding guarantee: immediately before AAA `on_idle` in every conforming block, both remaining Weight dimensions are at least this value. The runtime MUST enforce the guarantee through block dispatch limits, mandatory-hook maxima, and hook order. AAA MUST meter itself against `min(actual_remaining, AaaOnIdleReserve)` and MUST run before any lower-priority idle consumer that could spend the reserve. Failure to prove this property makes the embedding non-conforming.

`on_idle` order is:

```text
base
-> bounded saturated-FIFO cleanup
-> due temporal work
-> observation fanout
-> cache-revalidation worker, when active
-> cutoff snapshot
-> FIFO attempts / scheduler-owned automatic cleanup when execution gates are clear
-> starvation update
```

`OnIdleBaseWeightUpper` covers pallet entry, global breaker/revalidation reads, fixed worker orchestration, cutoff snapshot, and the maximum starvation-state read/update branch. Temporal and observation worker bases live inside their own limits. FIFO head discovery, consumption, attempt, final placement, and automatic cleanup live inside the admitted actor attempt-or-cleanup bound.

Every unit admits complete RefTime and ProofSize. Workers do not borrow actor service.

```text
ActorServiceReserve =
    AaaOnIdleReserve
    - OnIdleBaseWeightUpper
    - SaturatedQueueCleanupWeightUpper
    - WakeupWeightLimit
    - ObservationFanoutWeightLimit
```

Subtraction is checked component-wise; underflow is invalid configuration. One maximum actor attempt-or-cleanup or cache-revalidation unit MUST fit this derived reserve; plan/suffix admission compares against it. `MaxExecutionsPerBlock` is only a count ceiling and makes no throughput claim.

While the global breaker is active:

- FIFO attempts and scheduler-owned automatic cleanup do not run;
- Bounded queue/temporal/observation housekeeping continues;
- Creation, System locator reuse, and activation fail with `GlobalCircuitBreakerActive`;
- Every otherwise authorized control over an existing actor remains available subject to mutability and control-mutation limits; it MAY establish readiness, but no attempt executes until the breaker clears;
- Manual latch, address/observation ingress, control/ingress-triggered terminal close, explicit Mutable close, sweep, breaker control, and active-limit control remain available.

Cache revalidation is an independent global execution gate; it does not require or mutate the public breaker.

```text
CurrentCacheEpoch: CacheEpoch
CacheRevalidation: Option<CacheRevalidationState<Cursor>>
ActorHot.cache_epoch: CacheEpoch
```

A cache-affecting runtime upgrade checked-increments `CurrentCacheEpoch`, creates one durable `CacheRevalidationState`, and snapshots the exact Active workset through a generated bounded cursor. While the state exists:

- No FIFO attempt or scheduler-owned automatic cleanup runs;
- FIFO discovery is not entered, so the head is neither demoted nor classified `BlockedOther`;
- Starvation state is frozen and no detection/recovery event is caused by revalidation;
- Active creation, activation, and semantic execution-plan replacement fail with `CacheRevalidationActive`; Dormant creation does not grow the workset and remains available;
- Close and deactivation remain available and remove the actor from the remaining workset;
- Each bounded worker unit revalidates at most its admitted count/Weight, stamps a surviving actor with `target_epoch`, and advances durable cursor/remaining state atomically.

The workset cannot grow. Missing/closed actors are skipped. After every surviving Active actor is stamped and `remaining == 0`, the state clears atomically; ordinary FIFO service resumes in the next block without reticketing, so preexisting order is preserved. The publicly readable state supplies progress observability. Physical cursor encoding belongs to generated storage descriptors.

`permissionless_sweep` is O(1); batch sweep is O(K <= MaxSweepBatch), performs terminal cleanup only, and ignores missing batch ids. Starvation changes only on `BlockedByWeight` with no admitted attempt and writes on transitions. Reaching `MaxIdleStarvationBlocks` enters `Alerted` and emits `IdleStarvationDetected` once; clearing the condition emits `IdleStarvationRecovered` once. The threshold triggers no breaker, priority, reordering, forced execution, or other authority change.

## 6. Host Adapters and Weight

### 6.1 Failure Contract

```rust
enum RetryClass { Permanent, Temporary }
struct TaskFailure { error: DispatchError, retry: RetryClass }
```

Retryability never derives from strings, module indices, broad token errors, or raw coincidence. Unknown is Permanent.

Temporary includes dynamic slippage, authored-cap insufficiency against a current quote, reference freshness/deviation, liquidity/output/ratio movement, recipient deposit unavailability, and recoverable queue/wakeup capacity or placement unavailability reached through paired ingress.

Permanent includes unsupported/malformed configuration, invalid provenance, missing static capability, monotonic ticket/index namespace exhaustion, topology corruption, and invariant failure.

`FundingUnavailable` is a resolution outcome, not `TaskFailure`. Successful adapters remain within declared effects and debit surfaces.

### 6.2 Interfaces

```rust
struct ExecutionContext<'a, A> {
    actor: &'a A,
    aaa_type: AaaType,
}

struct IngressFailure {
    error: DispatchError,
    retry: RetryClass,
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
    fn swap_exact_in(
        context: ExecutionContext<'_, A>,
        asset_in: I,
        asset_out: I,
        amount_in: B,
        tolerance: Perbill,
    ) -> Result<B, TaskFailure>;

    fn swap_exact_out(
        context: ExecutionContext<'_, A>,
        asset_in: I,
        asset_out: I,
        amount_out: B,
        authored_input_cap: B,
        tolerance: Perbill,
    ) -> Result<B, TaskFailure>;
}

trait StakingOps<A, I, B> {
    fn stake(who: &A, asset: I, amount: B) -> Result<(), TaskFailure>;
    fn unstake(who: &A, asset: I, shares: B) -> Result<(), TaskFailure>;
    fn share_balance(who: &A, asset: I) -> B;
    fn share_asset(asset: I) -> Option<I>;
}

trait LiquidityOps<A, I, B> {
    fn lp_assets(lp: I) -> Option<(I, I)>;
    fn add_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B, min_lp: B) -> Result<(B, B, B), TaskFailure>;
    fn remove_liquidity(who: &A, lp: I, a: I, b: I, lp_amount: B, min_a: B, min_b: B) -> Result<(B, B), TaskFailure>;
    fn donate_liquidity(who: &A, a: I, b: I, max_a: B, max_b: B, max_ratio_error: Perbill) -> Result<(B, B), TaskFailure>;
}

trait FeeCollector<A, I, B> {
    fn collect_fee(payer: &A, sink: &A, fee_asset: I, amount: B) -> DispatchResult;
}

enum ScalarObservationState<N> {
    Unavailable,
    Uninitialized,
    Fresh { value: u128, observed_at: N },
    Stale,
}

trait ObservationProvider<F, N> {
    fn observe(feed: &F, now: N, max_age: u32) -> ScalarObservationState<N>;
}

trait FundingAuthority<A> {
    fn permits(
        aaa_id: AaaId,
        owner: &A,
        source: Option<&A>,
        provenance: Option<&FundingProvenance>,
    ) -> bool;
}

trait SovereignAccountPolicy<A> {
    fn is_reserved(account: &A) -> bool;
}

trait AddressEventIngress<A, I, B> {
    fn preflight(event: &AddressEvent<A, I, B>) -> Result<(), IngressFailure>;
    fn notify(event: &AddressEvent<A, I, B>) -> Result<(), IngressFailure>;
}

trait ObservationChangeIngress<F> {
    fn note_observation_changed(feed: F, revision: ObservationRevision) -> DispatchResult;
}
```

`AssetOps.balance` is the pre-reservation ordinary balance surface. Mint is System-only; transfer preflight models withdrawal, recipient deposit, and any certified destination ingress consequence. Staking-share and ordered LP bindings are admitted-plan identity, MUST NOT be reinterpreted in place, and change only through Section 9.4.

`DexOps` is an encapsulated quote-and-execute boundary. It MUST obtain the current executable quote internally before mutation, apply the Section 3.4 tolerance/cap formulas and Section 6.3 System guard, execute through Router, validate actual returned amounts, and return actual output/input. No caller-visible quote method or stale caller-supplied quote exists.

Liquidity success uses positive amounts within supplied caps/minima and no undeclared debit. For DonateLiquidity, AAA derives `max_b` exactly as specified in Section 3.4; `LiquidityOps` may choose actual amounts within both caps but MUST NOT derive or exceed another cap.

Valid `Fresh` requires `observed_at <= now` and age `<= max_age`; invalid Fresh is Permanent. Missing observation is `Unavailable`; missing funding authority denies. Every missing mutation capability fails closed.

### 6.3 System Swap Guard and Router Boundary

AAA supplies resolved intent, class, finite authored/custody caps, tolerance, and failure policy. `DexOps` owns current quote acquisition, quote protection, the AAA-specific System reference guard, Router invocation, and actual-amount validation. Router owns route discovery, path validation, Router fees/protection, Oracle publication, execution, and route outcome. AAA neither defines nor alters route semantics.

For User context, `DexOps` MUST NOT apply System reference parameters. For System context, before mutation and again against returned actual amounts when they may differ from the executable quote, it requires fresh nonzero directed reference values and enforces the one-sided minimum execution rate:

```text
exec_out * ref_in * Perbill::ACCURACY
    >=
(Perbill::ACCURACY - SystemSwapMaxReferenceDeviation.deconstruct())
    * ref_out
    * exec_in
```

All products use widened checked arithmetic. A better-than-reference execution always passes. Missing/stale/zero reference or worse execution beyond the bound is Temporary and rolls back the task layer. Router does not own or reinterpret the System guard.

### 6.4 Weight, Fee, and Cache Authority

One generated `WeightInfo` owns calls, task classes, condition/amount evaluation, fee collection, ingress, scheduler/observation/revalidation units, probes, orchestration, Continuation, finalization, events, and cleanup. No parallel `TaskWeightInfo`, client numeric table, divergent fallback, silent production default, or unexplained raw orchestration literal may exist.

Task Weight covers internal quote, tolerance arithmetic, System guard, Router/adapter work, paired ingress, rollback, actual validation, and success event. Step evaluation Weight covers every condition, amount preparation, fee collection, and non-task outcome. Attempt Weight adds probes, opening snapshots, finalization, future placement/close, and cleanup. Composition overflow is `AdmissionBoundOverflow`. Simulation and fee conversion use the same authority.

`cycle_weight_upper` and `cycle_fee_upper` are checked full-plan projections, not independent authorities:

```text
cycle_weight_upper =
    derive_weight(actor_class, execution_plan, current Weight/adapter/envelope bindings)

cycle_fee_upper =
    derive_fee(actor_class, execution_plan, current WeightToFee bindings)
```

Fresh-cycle admission or client projection MAY read a cache only while `cache_epoch == CurrentCacheEpoch` and storage invariants prove equality with the current derivation. Suffix admission derives the current suffix bound. No attempt may admit, reserve, charge, or report from a stale cache.

A cache-affecting runtime upgrade follows the canonical global/per-actor representation and execution gate in Section 5.4. Before ordinary attempts resume, its bounded worker MUST apply the Section 9.4 Continuation policy, recompute both caches, rerun complete plan admission against `ActorServiceReserve`, stamp the target epoch, and atomically advance durable progress. Partial revalidation exposes no actor to execution.

The migration-specific contract MUST define the disposition of any actor that no longer admits; absent such a disposition, the runtime change MUST NOT activate. Every cache-affecting change requires production evidence, interruption/resume tests, and exact cache-equality checks.

## 7. Calls and Simulation

### 7.1 Calls

```text
create_user_aaa                  create_user_aaa_at_slot
create_system_aaa                create_system_aaa_at_sovereign_id
activate_aaa / deactivate_aaa    pause_aaa / resume_aaa
manual_trigger                    close_aaa
update_schedule                   update_execution_plan
update_funding_source_policy      set_auto_close_at_cycle_nonce
increment_auto_close_nonce        cancel_continuation
set_global_circuit_breaker        set_active_actor_limit
permissionless_sweep              permissionless_sweep_many
```

Authorization is:

| Call group | Required origin |
| --- | --- |
| User creation | signed caller, who becomes owner |
| System creation, System locator reuse, active-limit control | `SystemOrigin` |
| breaker control | `GlobalBreakerOrigin` |
| User actor control | signed owner |
| System actor control | signed owner or `SystemOrigin` |
| sweep | any signed origin |

Actor control includes `manual_trigger`. It requires Active, unpaused, nonexpired state and an authored Manual source. User Immutable MAY use it; System Immutable cannot author Manual. It sets `pending_signal` only `false -> true`, atomically ensures one future path, and emits `ManualTriggerSet` only for that latch transition. A previously latched actor with a valid path changes nothing.

When `cycle_state == Suspended`, Manual only latches readiness for the next logical cycle. It does not cancel, replace, accelerate, duplicate, or retarget the current Continuation retry path. Manual is exempt from the membership rate limit in Section 2.4.

Behavior is otherwise owned by Sections 2-5. Exact no-op returns before mutation/event. Calls that may close include cleanup Weight. Creation fails with `AaaIdOccupied` when `NextAaaId` already owns any identity/Active partition. Active creation/activation and semantic plan replacement fail with `CacheRevalidationActive` while Section 5.4 revalidation is active; Dormant creation remains available. Active creation/activation respects `ActiveActorLimit`; all creation respects `MaxActorIdentities`; fresh System locator creation respects `MaxSystemSovereigns`.

User Active creation and User activation require the prospective/current sovereign fee-native balance to cover `MinUserBalance + attempt_fee_envelope(plan, 0, User).total` before the opening fee or Active state commits; failure returns `InsufficientBalance`. The unfunded lifecycle is therefore `create Dormant -> fund the deterministic sovereign account -> activate`. A later resource shortfall at attempt opening remains a terminal predicate under Section 2.4. Auto-close `Some(t)` requires `1 <= t - cycle_nonce <= MaxAutoCloseNonceHorizon`; increment requires `by > 0` and the same horizon. Active-limit update requires `ActiveAaaCount <= limit <= min(MaxActiveActors, MaxQueueLength)` and nonzero. System account derivation is a pure helper; no recovery-transfer call exists. Breaker behavior follows Section 5.4.

### 7.2 Simulation

```rust
enum SimulationMode { FreshCurrentPlan, CurrentContinuation }
enum SimulationStatus { Completed, Aborted, Suspended, Closed(CloseReason) }
enum SimulationStepOutcome { Executed, Skipped(StepSkippedReason), Failed(RetryClass), Suspended(SuspensionReason), Stopped }
struct SimulationStepRecord { step_index: u32, outcome: SimulationStepOutcome }
enum SimulationError {
    ActorNotFound, ProgramMismatch, TypeMismatch, MutabilityMismatch,
    ModeCycleStateMismatch, CacheRevalidationActive, GlobalCircuitBreaker,
    WindowExpired, Paused, CycleNonceExhausted, ConsecutiveFailures,
    NotReady, BalanceUnavailable, FeeBudgetUnavailable,
    ContinuationInvariant, TransactionDepthExceeded,
}
struct SimulationResult {
    status: SimulationStatus,
    cycle_nonce: u64,
    attempt: u32,
    start_cursor: u32,
    continuation_cursor: Option<u32>,
    unsuccessful_attempts_at_cursor: Option<u32>,
    finalized_through: Option<u32>,
    cumulative_outcomes: OutcomeTotals,
    steps: BoundedVec<SimulationStepRecord, MaxExecutionPlanSteps>,
}
trait AaaSimulationApi<Program> {
    fn simulate_current_program(
        aaa_id: AaaId,
        expected_type: AaaType,
        expected_mutability: Mutability,
        expected_program: Program,
        mode: SimulationMode,
    ) -> Result<SimulationResult, SimulationError>;
}
```

One call simulates exactly one fresh or Continuation attempt; therefore `steps.len() <= execution_plan.len() - start_cursor <= MaxExecutionPlanSteps`. The concrete runtime API binds the same runtime `MaxExecutionPlanSteps` used by execution.

Status mapping is exact:

- `Completed`: the simulated cycle reaches successful terminal and remains open.
- `Aborted`: the simulated cycle reaches terminal `CycleResult::Failed` without suspension or close.
- `Suspended`: the simulated attempt would persist Continuation.
- `Closed(reason)`: the simulated transition would close with `reason`, including completion- or failure-driven close.

When multiple errors apply, simulation returns the first item in this order:

```text
TransactionDepthExceeded
ActorNotFound
TypeMismatch
MutabilityMismatch
ProgramMismatch
ModeCycleStateMismatch
ContinuationInvariant
CacheRevalidationActive
GlobalCircuitBreaker
WindowExpired
Paused
CycleNonceExhausted
ConsecutiveFailures
NotReady
BalanceUnavailable
FeeBudgetUnavailable
```

`ActorNotFound` includes Dormant identity because no Active program exists. An old `expected_program` after semantic replacement returns `ProgramMismatch`; supplying the current program with `CurrentContinuation` after cancellation returns `ModeCycleStateMismatch`. A Suspended marker with absent/malformed Continuation returns `ContinuationInvariant`.

Simulation executes in rollback-only storage, uses production condition/amount/fee/task/control/finalization logic, persists no state/event, and owns no separate semantic or Weight model.

## 8. Events and Ordering

```rust
AaaCreated { aaa_id: AaaId, owner: AccountId, actor_class: ActorClass, mutability: Mutability, sovereign_account: AccountId, initial_lifecycle: InitialLifecycle }
AaaActivated { aaa_id: AaaId }
AaaDeactivated { aaa_id: AaaId }
AaaPaused { aaa_id: AaaId, reason: PauseReason }
AaaResumed { aaa_id: AaaId }
AaaClosed { aaa_id: AaaId, reason: CloseReason }
CycleStarted { aaa_id: AaaId, cycle_nonce: u64 }
CycleSummary { aaa_id: AaaId, cycle_nonce: u64, result: CycleResult, executed_steps: u32, committed_effectful_tasks: u32, skipped_conditions: u32, skipped_resolution: u32, skipped_funding_unavailable: u32, failed_steps: u32 }
StepSkipped { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, reason: StepSkippedReason }
StepFailed { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, retry_class: RetryClass, error: DispatchError }
TransferExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance, to: AccountId }
SplitTransferExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, total: Balance, distributed: Balance, retained: Balance, legs: u32, effective_legs: u32 }
SwapExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance }
BurnExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
MintExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
StakeExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, amount: Balance }
UnstakeExecuted { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset: AssetId, shares: Balance }
LiquidityDonated { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, max_amount_a: Balance, max_amount_b: Balance, amount_a: Balance, amount_b: Balance }
LiquidityAdded { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance, lp_minted: Balance }
LiquidityRemoved { aaa_id: AaaId, cycle_nonce: u64, step_index: u32, lp_asset: AssetId, lp_amount: Balance, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance }
ScheduleUpdated { aaa_id: AaaId }
ExecutionPlanUpdated { aaa_id: AaaId, completion_policy: CompletionPolicy }
AutoCloseNonceSet { aaa_id: AaaId, target: Option<u64> }
AutoCloseNonceIncremented { aaa_id: AaaId, old_target: Option<u64>, new_target: u64, by: u64 }
ActiveActorLimitSet { old_limit: u32, new_limit: u32 }
GlobalCircuitBreakerSet { paused: bool }
ManualTriggerSet { aaa_id: AaaId }
SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 }
IdleStarvationDetected { consecutive_blocks: u32 }
IdleStarvationRecovered { consecutive_blocks: u32 }
FundingSourcePolicyUpdated { aaa_id: AaaId }
FundingAccumulated { aaa_id: AaaId, asset: AssetId, added: Balance, accumulated: Balance }
CycleSuspended { aaa_id: AaaId, cycle_nonce: u64, attempt: u32, cursor: u32, reason: SuspensionReason, cumulative_outcomes: OutcomeTotals }
CycleContinued { aaa_id: AaaId, cycle_nonce: u64, attempt: u32, cursor: u32 }
CycleCancelled { aaa_id: AaaId, cycle_nonce: u64, reason: CancellationReason }
CycleStopped { aaa_id: AaaId, cycle_nonce: u64, step_index: u32 }
```

Event durability follows Section 4.4. `ManualTriggerSet` emits only on latch `false -> true`. `FundingAccumulated` emits only for a tracked, policy-accepted credit. Cycle key is `(aaa_id, cycle_nonce)`; attempt adds `attempt`.

Opening consumes funding and emits `CycleStarted`, then step/effect events, optional `CycleStopped`, then suspension or summary. Retry begins with `CycleContinued`, emits no second start, and consumes no second snapshot. Completed/Failed finalization emits summary then optional close.

Cancellation emits `CycleCancelled`, then `CycleSummary(Cancelled)`. If cancellation is caused by close, `AaaClosed` follows. If caused by semantic update or deactivation, the corresponding update/deactivation event follows. Pure close means no Continuation exists and emits only `AaaClosed`. Weight/scan deferral emits nothing. Unbounded history is off-chain.

## 9. ABI, Errors, Storage, and Upgrades

### 9.1 ABI Authority

Sections 1-8 own semantic meaning. Exact indices, discriminants, field order, bounds, and runtime API encoding come only from complete generated metadata. Before launch, accepted cleanup is one reviewed epoch; unexplained reserved indices/compatibility shadows are non-conforming. After launch, existing public shapes are immutable; additions append or use a new typed wrapper.

### 9.2 Errors

```rust
enum Error {
    AaaIdOverflow,
    AaaNotFound,
    ActiveAaaCapacityExceeded,
    ActiveAaaCountInvariant,
    ActorIdentityCapacityExceeded,
    ActorIdentityCountInvariant,
    AaaAlreadyActive,
    AaaDormant,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    ActiveAaaLimitBelowCurrent,
    AaaPaused,
    EmptyExecutionPlan,
    ExecutionPlanExceedsOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    CacheRevalidationActive,
    ImmutableAaa,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidCondition,
    InvalidAutoCloseNonce,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    SelfTransferNotAllowed,
    InvalidTriggerConfiguration,
    MintNotAllowedForUserAaa,
    NotGovernance,
    NotOwner,
    NotPaused,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    InvalidOwnerSlot,
    AaaIdOccupied,
    SystemSovereignCapacityExceeded,
    SystemSovereignUnknown,
    SystemSovereignOccupied,
    ExecutionPlanTooLong,
    SnapshotUnavailable,
    FundingAccumulatorOverflow,
    SovereignAccountCollision,
    ReservedSovereignAccount,
    QueueTicketExhausted,
    SchedulerIndexExhausted,
    SystemSovereignInvariant,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceOverflow,
    AutoCloseNonceIncrementZero,
    ControlMutationRateLimited,
    QueueCapacityUnavailable,
    RetryLaterNotAllowedForImmutableAaa,
    ContinuationNotFound,
    ContinuationInvariant,
    EmptyConditionSet,
    ManualSourceDisabled,
    InvalidTradeBound,
    InvalidRetryAttemptLimit,
    RecipientDepositUnavailable,
    InvalidObservationMaxAge,
    ObservationSubscriptionCapacityExceeded,
    ObservationSubscriptionInvariant,
    InvalidObservationRevision,
    DirtyObservationCapacityExceeded,
    DirtyObservationInvariant,
    AdmissionBoundOverflow,
}
```

Resolution outcomes are not pallet `Error` variants. `TaskFailure` is the adapter failure envelope; its `error` MAY carry a pallet `Error` solely as a stable diagnostic code, without converting the step into an extrinsic-level rejection.

`RecipientDepositUnavailable` is used only in that execution-time diagnostic role and is classified Temporary. `InsufficientFee` belongs only to User creation-fee collection. `AaaIdOccupied` belongs only to creation against an already-owned `NextAaaId`. `NotGovernance` applies when an origin accepted by `SystemOrigin` targets a User actor. `ControlMutationRateLimited` belongs to Section 2.4. Unknown adapter failure remains Permanent. Every extrinsic-level pallet error is selected by its owning section.

### 9.3 Storage Contract

Generated storage descriptors own exact prefixes, hashers, keys, values, and physical topology. Normatively required are: Section 2 partitions; exact slot/locator/reverse-index/subscription/readiness ownership; persistent identity control clock; `CurrentCacheEpoch`; optional durable `CacheRevalidationState`; per-Active `cache_epoch`; one live ticket and at most one ordinary temporal target per actor subject to Section 5.1 terminal exception; bounded collections and exact counters/reverse ownership; canonical ordered encoding; no unbounded execution history; and `try_state` reconciliation of ownership, cardinality, membership, revision, revalidation progress, cache equality, and independent processing bounds.

The canonical initial state contains no migration, dual write, legacy decoder, bridge, stale alias, or compatibility storage. It initializes one defined `CurrentCacheEpoch`, stamps every configured Active actor with that epoch, leaves `CacheRevalidationState` absent, initializes a nonzero `ActiveActorLimit` satisfying Section 10, validates `SystemSovereignCount <= MaxSystemSovereigns`, and reconciles every configured System actor/custody locator, identity counter, reverse index, subscription, cache, and initial scheduler path. These values MAY be derived from runtime bindings or providers; the generated genesis descriptor owns their physical representation.

### 9.4 Post-Launch Migration

Each storage change, semantic state rewrite, or runtime-upgrade override of an Immutable actor first ships a migration-specific specification defining source and target schemas/semantics, bounded work unit, durable progress owner, Weight per invocation, failure/resume/completion behavior, semantic mapping, custody consequences, terminal invariant, and Continuation `Cancel | PreserveWithProof` policy. Idempotence means that after target completion, later invocations perform no semantic mutation. Partial migration cannot expose reinterpreted state to ordinary execution. Continuation preservation additionally proves equivalent program, frozen inputs, failures, fees, Weight, and eligibility; otherwise cancel. Cache-affecting runtime changes additionally satisfy the bounded revalidation protocol in Section 6.4.

## 10. Runtime Configuration and Release Gates

### 10.1 Relations

Required bindings include `AaaPalletId`, `FeeNativeAssetId`, `SystemOrigin`, `GlobalBreakerOrigin`, adapters/services, `FeeSink`, `AaaCreationFee`, `WeightToFee`, one `WeightInfo`, `AaaOnIdleReserve`, generated base/cleanup bounds, worker/revalidation limits, `MaxSweepBatch`, independent queue/wakeup/observation processing bounds, System reference parameters, and `TargetBlockTime`.

1. `0 < ActiveActorLimit <= min(MaxActiveActors, MaxQueueLength)`; after update, `ActiveActorLimit >= ActiveAaaCount`.
2. `MaxActorIdentities >= MaxActiveActors`.
3. `MaxSystemSovereigns > 0`; every count/index type represents its configured bound.
4. `0 < MaxOwnerSlots <= 255`.
5. `0 < MaxExecutionPlanSteps <= 255`.
6. `MaxRetryAttempts >= 2`.
7. `MaxExecutionPlanSteps * MaxRetryAttempts <= u32::MAX`, evaluated with checked arithmetic.
8. `MaxConsecutiveFailures > 0`.
9. `MaxOpeningSnapshotEntries == 2 * MaxExecutionPlanSteps`, evaluated with checked arithmetic; persisted funding entries fit `MaxFundingTrackedAssets`.
10. `MaxActiveActors.checked_mul(MaxTriggerSources)` is representable in `u32`; total subscriptions, distinct subscribed feeds, and dirty obligations obey Section 5.2 derived bounds.
11. `QueuePageSize`, `WakeupPageSize`, and `ObservationPageSize` are independently named and nonzero even when configured values are equal.
12. `MaxSweepBatch > 0`.
13. Every collection, scan, attempt, worker, revalidation, and processing-unit bound is nonzero and has one owner.
14. Each worker limit covers one complete worst-case unit.
15. The embedding guarantees `remaining_weight_at_AAA_on_idle >= AaaOnIdleReserve` in both dimensions.
16. Section 5.4 subtraction is representable in both Weight dimensions and `ActorServiceReserve` covers one maximum actor attempt-or-cleanup or one maximum revalidation unit.
17. Every admitted plan/suffix, producer consequence, control transition, and cleanup fits its owning envelope.
18. `MinUserBalance >= AssetOps::minimum_balance(FeeNativeAssetId)`; `AaaCreationFee > 0`; `WeightToFee` maps every admitted User evaluation/task upper bound to a positive fee.
19. `SystemSwapEmaMaxAgeBlocks > 0`; `SystemSwapMaxReferenceDeviation < Perbill::one()`.
20. `TargetBlockTime > 0`.
21. `MaxExecutionDelayBlocks = ceil(10 Julian years / TargetBlockTime)`; target arithmetic and checked `end + 1` are representable; window length is inclusive.
22. `MaxAutoCloseNonceHorizon > 0`.
23. `MaxIdleStarvationBlocks > 0`; `MaxTimerJitterBlocks` MAY be zero.
24. Simulation records are bounded by the same `MaxExecutionPlanSteps` as one execution attempt.
25. Every loop/storage bound appears in metadata or generated descriptors.
26. `CurrentCacheEpoch.checked_add(1)` is representable before a cache-affecting upgrade.
27. While `CacheRevalidationState` exists, no actor attempt or automatic cleanup executes, creation/activation/plan replacement is rejected, and starvation accounting is unchanged.
28. For every executable Active actor, `cache_epoch == CurrentCacheEpoch` and `cycle_weight_upper` / `cycle_fee_upper` equal their current Section 6.4 derivations.

Structural-bound or cache-derivation changes regenerate metadata/storage/Weight evidence, replay the model, and complete bounded Active revalidation before attempts resume.

### 10.2 Semantic Reference Profile

```text
TargetBlockTime = 6 seconds
MaxActiveActors = 10_000                 MaxOwnerSlots = 255
MaxExecutionPlanSteps = 8                MaxRetryAttempts = 10
MaxFundingTrackedAssets = 10             MaxOpeningSnapshotEntries = 16
MaxConditionsPerStep = 4                 MaxTriggerSources = 4
MaxWhitelistSize = 16                    MaxSplitTransferLegs = 8
MaxConsecutiveFailures = 10              MaxQueueLength = 10_000
MaxExecutionDelayBlocks = 52_596_000     MaxTimerJitterBlocks = 64
MinWindowLength = 100                    MaxAutoCloseNonceHorizon = 10_000
MaxIdleStarvationBlocks = 25             MaxSweepBatch = 5
MinUserBalance = 5 * existential deposit
AaaCreationFee = existential deposit
```

`52_596_000` is ten Julian years at six seconds. The reference profile exposes 255 User slots and reserves bitmap bit `255` as the permanently invalid `u8::MAX` sentinel.

### 10.3 Measured Runtime Bindings

`QueuePageSize`, `WakeupPageSize`, `ObservationPageSize`, `MaxQueueEntriesScannedPerBlock`, `MaxExecutionsPerBlock`, `MaxWakeupsPerBlock`, `MaxObservationFanoutPagesPerBlock`, revalidation unit/count bounds, `WakeupWeightLimit`, `ObservationFanoutWeightLimit`, `AaaOnIdleReserve`, base/cleanup bounds, and adapter worst cases are measured runtime outputs. Their accepted values live in generated runtime/Weight descriptors, not in this specification. No count ceiling or worker percentage is a throughput guarantee.

### 10.4 Gates

One evidence identity binds specification, commit, production Wasm, metadata/storage descriptors, weights, semantic manifest, fee vectors, client descriptors, certified producer/publisher inventories, and benchmarks. Acceptance requires:

- Exact rollback tests for every control, attempt, late placement, subscription, cleanup, fee, quote/guard, and producer failure boundary;
- Model proof of Section 2.4 failure streak, FundingUnavailable/Temporary suspension counters, cursor-local backoff reset, Active-epoch/reset table, activation nonce sequence, persistent identity control-mutation clock, and Section 3.4 step matrix;
- Metadata/model tests proving only the canonical `PercentageAtOpening`, policy-typed `OpeningSurface`, `AllAvailable`, `CycleState`, `cycle_state`, `opening_snapshot`, `MaxOpeningSnapshotEntries`, and `ControlMutationRateLimited` names exist; no compatibility alias remains;
- Authorization and breaker matrices covering every call in Section 7.1;
- Exhaustive amount tests for policy-typed current/opening surfaces, absence of signal-payload amount resolution, ordinary balance subjects, ordinary balance versus staking shares, exact `spendable_balance`/`preservable_balance`, multi-surface precedence, forbidden output-target `AllAvailable`, exact Stake/Unstake/RemoveLiquidity debits, AddLiquidity/DonateLiquidity caps, DonateLiquidity derived `max_amount_b`, deterministic SwapOut `authored_cap/effective_max_in`, internal quote ownership, full slippage tolerance, self-recipient rejection, zero-distribution `ResolutionSkipped`, and one-effective-leg SplitTransfer;
- Fee vectors proving the exact `R_i -> R_(i+1)` reservation transition for every condition/resolution/task outcome, retry, fee-collector failure, and enclosing rollback;
- Funding/opening tests proving pre-clear snapshot capture, atomic accumulator clear, `pending_signal -> false`, exact `CycleStarted` nonce, no consumption on failed admission, suffix-only Continuation projection, and balance-only untracked/policy-rejected credits while tracked overflow rolls back the producer;
- Certified producer inventory tests proving AAA Transfer/SplitTransfer/Mint and every other named movement call preflight/move/notify exactly once in one transaction, recoverable placement failure is Temporary, unrecoverable namespace/invariant failure is Permanent, certified failure rolls back movement, and every unnamed movement is balance-only;
- Certified observation publisher tests proving origin/owner, revision monotonicity, first-subscriber semantics, O(1) publication, and absence of AAA effects from uncertified publication paths;
- User Active creation/activation prefunding tests and Dormant-fund-activate flow;
- Simulation ABI tests proving bounded records, the complete ordered error precedence, cache-revalidation gating, semantic-replacement cancellation behavior, and one-attempt equivalence with production;
- Ingress terminal-owner tests; terminal-order scope per owning transition; exact Immediate/Cadenced temporal-eligibility/window formulas; cursor-local retry timing; phase-only jitter/cadence-grid tests; Immediate/WhenSignalled no-self-signal tests; optional post-cycle placement/no-placement cases; ordinary/terminal temporal coexistence, marker ownership, and terminal-first precedence; Manual-during-Continuation tests; close/update/deactivation event-order tests with and without Continuation;
- One numeric Weight authority, Weight-derived evaluation fees, quote/guard/Router inclusion in task Weight, one-sided adapter-owned System guard, global epoch/per-actor cache stamps, deterministic bounded revalidation cursor/progress, no-FIFO-discovery revalidation gating, interruption/resume and no-longer-admitted disposition tests, proof that every block supplies `AaaOnIdleReserve`, all Section 10 relations, and the Section 5.4 reserve under production benchmarks;
- Independent embedding/Executive proof of lifecycle, `b"user"` / `b"system"` custody-domain separation, deterministic custody across host account reaping, protected-minimum debit behavior, fees, certified producer inventory, Weight correction, no duplicate ingress, and the step-local diagnostic role of `RecipientDepositUnavailable`;
- State-machine coverage of locators, no-ops, retries, funding, cutoff, HOL, temporal readiness, breaker, observation churn, corruption, exhaustion, terminal precedence, and immutable indefinite commitments;
- Generated client surfaces with no independent semantic/numeric authority;
- Quantified cost/account count to fill actor/identity/scheduler capacity and explicit runtime-upgrade recovery path/expected governance latency for call-level unstoppable System Immutable actors; AAA claims no adaptive anti-spam pricing beyond configured fees/limits;
- A saturated-service model quantifying FIFO traversal and eligible-to-attempt latency from `MaxActiveActors`, `MaxExecutionsPerBlock`, worker envelopes, one-attempt-per-actor-per-block, and target block time; cadence documentation MUST NOT imply periodic service;
- Genesis/fixtures containing only canonical state, a valid initial `ActiveActorLimit`, bounded System locator cardinality, fresh cache derivations, and no compatibility shadow.

Architecture is written only after these gates. It explains shared FIFO, semantic profile choices, call-level immutable commitments and runtime-upgrade recovery, physical topology, measured runtime bindings, and the choice to keep simulation readiness-faithful without extending this contract.

---

_End of specification._
