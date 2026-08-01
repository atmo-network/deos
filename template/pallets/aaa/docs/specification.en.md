# AAA Specification

- **Scope**: Account Abstraction Actors runtime contract
- **Target**: `0.7.10`
- **Date**: July 2026
- **Status**: Normative

> **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** are interpreted as defined by RFC 2119 and RFC 8174 when written in uppercase.

Sections 1–11 define behavior, Section 12 public SCALE shape, Section 13 storage topology, Section 14 conformance, and Section 15 runtime bindings. Authoring formats are non-normative projections of the typed runtime model.

**Account Abstraction Actors (AAA)** are bounded runtime actors controlling deterministic sovereign accounts. AAA abstracts account behavior, not transaction-signature validation.

---

## 1. Stability Contract

1. **Determinism**: identical network state and block context MUST produce identical AAA behavior.
2. **Bounded work**: every hook and extrinsic MUST be O(1) or O(K) under explicit finite `Max*` bounds. Hook work MUST reserve complete `Weight(RefTime, ProofSize)` before mutation and stop before either dimension is exhausted.
3. **Static programs**: an execution plan is a bounded ordered `Step[]` with no loops, arbitrary jumps, dynamic successors, task-authored memory, nested programs, opaque dispatch, or whole-plan transaction. Continuation stores scheduler-owned progress only.
4. **Closed outcomes**: every admitted decision MUST resolve through a named completion, skip, failure, deferral, suspension, cancellation, or close path. Adapter, arithmetic, namespace, and capacity failures MUST fail closed.
5. **Atomicity boundary**: each task is atomic; the plan is non-atomic and preserves earlier committed tasks. Terminal cleanup is atomic and performs no task or balance movement.
6. **Destruction in place**: close removes actor-owned state and preserves sovereign balances. AAA MUST NOT refund, fan out, or otherwise move assets after close. A later actor may reattach to the same vacant sovereign locator through an explicit creation path, but it receives a fresh `aaa_id` and inherits no actor semantics.
7. **Economics**: both User creation calls charge a non-refundable opening fee. AAA has no recurring rent. Executable User steps are attempt-priced: task rollback does not refund the already collected upper-bound execution fee.
8. **Synchronous state**: slot, identity, sovereign-locator registry, reverse-index, cardinality, subscription, and scheduler-membership mutations MUST commit with the authorizing transaction.
9. **Arithmetic**: amounts, counters, identifiers, fees, Weight bounds, and capacities use checked arithmetic. Saturation is allowed only where explicitly defined. Amount resolution MUST NOT clamp silently.
10. **Execution context**: behavior MUST respect FRAME hook semantics and MUST NOT depend on unavailable context such as the current block hash.
11. **Deferred horizon**: first eligible execution MUST NOT be configurable beyond `MaxExecutionDelayBlocks`, representing ten years at the target block time.
12. **Compatibility epoch**:
    - Before launch, explicitly accepted breaking cleanup is allowed and no individual call, event, error, or enum index is pinned unless a documented external consumer requires it;
    - Launch freezes one complete metadata-derived public ABI manifest;
    - After launch, existing public indices, discriminants, field order, encoded semantics, and storage meaning are immutable. Calls and enum variants may append; struct evolution requires a new versioned wrapper or call/variant. Storage changes require incremented `StorageVersion` and bounded idempotent migration.
13. **Single ABI authority**: exact runtime metadata MUST generate the complete public ABI manifest. Handwritten numeric registries, isolated historical pins without an accepted consumer, and legacy decoders MUST NOT become a second authority.
14. **Conformance**: deployment MUST remain blocked until Section 14 evidence passes against production metadata and Wasm.

## 2. Actor Model

### 2.1 Instance

- **Execution Plan**: the bounded ordered list of configured steps.
- **Logical Run / Cycle**: one causally connected plan execution identified by `(aaa_id, cycle_nonce)`.
- **Attempt**: one admitted execution of the unresolved suffix identified by `(aaa_id, cycle_nonce, attempt)`; opening is `0`, first retry is `1`.
- **Continuation**: sparse scheduler-owned state for a suspended run. Cancellation terminates the run without reverting its committed prefix.
- **Sovereign locator**: the stable custody key from which a sovereign account is derived. User uses `(owner, owner_slot)`; System uses `system_sovereign_id`. Actor identity and custody identity are independent.
- **FeeNativeAsset**: the asset used for opening fees, User step fees, `MinUserBalance`, reservation, and fee collection. Staking remains the generic `Stake { asset, amount }` task.
- **Fee-native balance**: `fee_native_balance` equals `AssetOps::balance(sovereign_account, FeeNativeAsset)` before applying the transient AAA fee reservation or the protected-floor subtraction.

```rust
type OwnerSlot = u8;
type OwnerSlotBitmap = [u8; 32];
type SystemSovereignId = u64;

enum ActorClass {
    User { owner_slot: OwnerSlot },
    System { sovereign_id: SystemSovereignId },
}
enum ActiveLifecycle { Active, Paused(PauseReason) }
enum RunState { Idle, Suspended }

struct ActorIdentity<AccountId> {
    class: ActorClass,
    mutability: Mutability,
    sovereign_account: AccountId,
    owner: AccountId,
    cycle_nonce: u64,
}

struct ActorHot<BlockNumber, Balance> {
    lifecycle: ActiveLifecycle,
    auto_close_at_cycle_nonce: Option<u64>,
    schedule_anchor: BlockNumber,
    last_cycle_block: Option<BlockNumber>,
    last_control_queue_mutation_block: Option<BlockNumber>,
    queue_ticket: Option<QueueTicket>,
    wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
    terminal_at: Option<BlockNumber>,
    consecutive_failures: u32,
    pending_signal: bool,
    run_state: RunState,
    cycle_weight_upper: Weight,
    cycle_fee_upper: Balance,
    funding_tracked_count: u32,
}

struct ActorProgram {
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow>,
    completion_policy: CompletionPolicy,
    execution_plan: BoundedVec<Step, MaxExecutionPlanSteps>,
}

struct ActorFunding<AccountId, AssetId, Balance> {
    funding_source_policy: FundingSourcePolicy<AccountId>,
    funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
    funding_accumulated: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
}

struct ContinuationState<BlockNumber, AssetId, Balance> {
    cursor: u32,
    attempt: u32,
    unsuccessful_attempts_at_cursor: u32,
    last_attempt_block: BlockNumber,
    trigger_snapshot: BoundedBTreeMap<ResolutionSurface<AssetId>, Balance, MaxContinuationSnapshotEntries>,
    funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
    cumulative_outcomes: OutcomeTotals,
}

struct OutcomeTotals {
    executed_steps: u32,
    committed_effectful_tasks: u32,
    skipped_conditions: u32,
    skipped_resolution: u32,
    skipped_funding_unavailable: u32,
    failed_steps: u32,
}
```

An Active program MUST have `1..=MaxExecutionPlanSteps` steps. One runtime binding applies to User and System actors and MUST satisfy `0 < MaxExecutionPlanSteps <= 255`; the `0.7.10` baseline configures it to `8`. The count bounds program structure, scans, snapshots, and the maximum actor-unit envelope, but FIFO liveness is proved by production Weight admission rather than by step count alone. Changing the binding requires regenerated metadata and production weights, repeated `G-WASM` and `G-MODEL` evidence, and revalidation of every affected Active admission cache before execution resumes.

Actor graphs MAY compose workflows longer than one local plan. Every graph edge remains asynchronous and scheduler-mediated, adds its own admission and custody boundary and, for User actors, its own fee and minimum-balance surface; it preserves neither same-block continuity nor one-event-one-run cardinality.

Dormant identity stores no program or active-epoch state. Immutable actors MUST be created Active. Genesis, every creation path, activation, and plan update MUST validate that one maximum actor probe plus the admitted run or suffix plus possible pure cleanup fits the actor-service envelope within `GuaranteedOnIdleWeight`; otherwise admission fails with `ExecutionPlanExceedsOnIdleBudget`. Validation precedes opening-fee collection and mutation.

### 2.2 Classes and Mutability

- **User AAA** pays User fees and consumes one owner slot below `MaxOwnerSlots`.
- **System AAA** is governance-created with an explicit owner, pays no User fee, and consumes no User slot. `ActorClass` is the sole class authority. Actor class does not affect FIFO service order. The fee exemption provides no economic spam resistance against governance-authored automation: AAA bounds resource use and grants no FIFO priority, while governance owns the risk of creating excessive System workload.
- **Mutable** control may pause/resume, replace schedule/window, replace plan/completion policy, replace funding policy, activate/deactivate, cancel Continuation, close, and change the auto-close target.
- The signed owner controls either class; governance additionally controls System AAA.
- **User Immutable** rejects configuration, lifecycle, close, and Continuation-control calls with `ImmutableAaa`. A configured Manual source remains invocable by its owner because it changes readiness, not program semantics.
- **System Immutable** rejects every control extrinsic, including governance/root, and cannot contain a Manual source. Internal terminal transitions remain valid.
- `Immutable` means governance-immutable under the current runtime dispatch contract. A runtime upgrade can technically replace that contract, but doing so is a protocol change with different promises, not a permitted mutation of the existing actor. Reusing a locator after terminal close creates a new actor contract and does not extend or mutate the closed actor's immutability promise.
- An Immutable actor without an internally reachable terminal condition MAY remain Active indefinitely and occupy its User slot or System locator under the current contract. Creating such an actor is an explicit irreversible commitment at this protocol level.
- Inbound transfers remain valid regardless of mutability and do not imply funding authority.
- `RetryLater { max_attempts }` requires `1 <= max_attempts <= MaxRetryAttempts`, is forbidden for every Immutable actor, and MUST be checked at every program-admission path. `MaxExecutionPlanSteps * MaxRetryAttempts <= u32::MAX` MUST hold.

### 2.3 Sovereign Derivation, Slots, and IDs

Sovereign accounts are derived as follows:

1. User: `seed = Blake2_256(SCALE(AaaPalletId, owner, owner_slot))`.
2. System: `seed = Blake2_256(SCALE(AaaPalletId, b"system", system_sovereign_id))`.
3. `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`.

The configured `AccountId` decoder MUST be total and deterministic for every 32-byte seed. The System derivation is a pure public function of `system_sovereign_id`; the first System actor at a locator uses its own fresh `aaa_id` as that locator.

User slot rules:

- `OwnerSlot` is `u8`; `OwnerSlotBitmap` is a fixed 256-bit `[u8; 32]`; and `0 < MaxOwnerSlots <= 255`. A valid slot satisfies `owner_slot < MaxOwnerSlots`. The `0.7.10` baseline exposes slots `0..=254`; bit `255` remains zero.
- Slot `s` maps to byte `s / 8` and little-endian bit `s % 8`. Every bit at an index `>= MaxOwnerSlots` MUST be zero.
- `OwnerSlotBitmaps: Map<AccountId, OwnerSlotBitmap>` is the sole owner-slot authority; an all-zero bitmap is deleted.
- `create_user_aaa` deterministically selects the lowest free slot by scanning at most 32 bytes in ascending byte/bit order. `create_user_aaa_at_slot` validates and updates one exact bit.
- Closing a User actor clears its bit. A later exact-slot creation allocates a fresh `aaa_id` and derives the same sovereign account without inheriting the closed actor's nonce, program, history, funding state, or guarantees.
- System actors are slotless.

System sovereign-locator rules:

```rust
enum SystemSovereignState {
    Vacant,
    Occupied(AaaId),
}
```

1. `SystemSovereigns: Map<SystemSovereignId, SystemSovereignState>` is the persistent bounded registry of allocated System custody locators; `SystemSovereignCount <= MaxSystemSovereigns` counts its entries. `MaxSystemSovereigns` is lifetime locator capacity, not simultaneous-live actor capacity; `Vacant` entries remain allocated because deleting them would break guaranteed sovereign-address recovery.
2. `create_system_aaa` allocates a fresh `aaa_id`, requires registry capacity, sets `system_sovereign_id = aaa_id`, and inserts `Occupied(aaa_id)` atomically with actor identity.
3. Closing a System actor changes its locator from `Occupied(aaa_id)` to `Vacant`; deactivation leaves it occupied.
4. `create_system_aaa_at_sovereign_id(system_sovereign_id, owner, mutability, program)` requires an existing `Vacant` locator, allocates a fresh `aaa_id`, changes the locator to `Occupied(new_aaa_id)`, and creates a new actor over the same sovereign account.
5. A recreated System actor starts with `cycle_nonce = 0` and inherits only custody state already present on the sovereign account. It does not inherit the former actor's identity, owner, mutability, program, Continuation, funding delta, readiness, clocks, events, or guarantees.
6. Unknown and occupied locators fail with `SystemSovereignUnknown` and `SystemSovereignOccupied`; fresh-locator exhaustion fails with `SystemSovereignCapacityExceeded`.

Sovereign guards:

- `SovereignIndex` maps each active or dormant sovereign account to its current `aaa_id` and rejects live ownership collision with `SovereignAccountCollision`.
- `SovereignAccountPolicy` rejects runtime-controlled accounts with `ReservedSovereignAccount` in O(1).
- Cross-domain User/System hash collision after close: derivations use `blake2_256` over domain-separated inputs — User derives from `(PalletId, owner, slot)` and System from `(PalletId, "system", aaa_id)` — so an accidental cross-domain collision would require a second preimage in a 256-bit output space with distinct domain tags, which is cryptographically negligible. No stronger quantitative preimage claim is part of the protocol. No persistent reverse index over allocated locator accounts is maintained; `SovereignIndex` covers current Active/Dormant ownership only and is absent for closed vacant locators. Reattachment remains the only path authorized to regain control of residual custody at a vacant locator.
- Ordinary pre-existing balances, locks, reserves, dust, or third-party transfers are valid and do not constitute collision.
- Reattachment through either a User slot or System sovereign locator grants the fresh actor authority over every residual custody surface exposed by configured host adapters, including deposits received while no actor was attached.
- User slot reuse and System locator reuse preserve custody continuity only. `aaa_id` is always freshly allocated and never reused.

ID rules:

1. Every creation path allocates `aaa_id = NextAaaId` and checked-increments the allocator; exhaustion returns `AaaIdOverflow` without mutation.
2. Reusing a User slot or System sovereign locator MUST NOT rewind or select `NextAaaId`.
3. Because every recreated actor receives a fresh `aaa_id`, `(aaa_id, cycle_nonce)` cannot repeat even when several actors sequentially control the same sovereign account.

### 2.4 Lifecycle

Terminal conditions and precedence:

| Condition | Result |
| --- | --- |
| `current_block > window.end` at any lifecycle touchpoint | `AaaClosed(WindowExpired)` before any other requested effect |
| User `fee_native_balance < MinUserBalance` before opening | `AaaClosed(BalanceExhausted)` |
| User `available_fee_budget < admission_fee_upper(cursor)` | `AaaClosed(FeeBudgetExhausted)` |
| stored `cycle_nonce == u64::MAX` before a new run | `AaaClosed(CycleNonceExhausted)` for either class |
| cursor-local unsuccessful attempts reach `max_attempts` | `AaaClosed(RetryAttemptsExhausted)` |
| `consecutive_failures >= MaxConsecutiveFailures` | `AaaClosed(ConsecutiveFailures)` |
| completed productive run under `CloseAfterProductiveRun` | `AaaClosed(ProductiveRunCompleted)` |
| otherwise completed run reaches auto-close target | `AaaClosed(AutoCloseNonceReached)` |

For a Temporary or `FundingUnavailable` `RetryLater` outcome, runtime MUST compute the post-attempt counters before writing state:

```text
next_local  = 1 when no prior cursor-local counter exists or the unresolved cursor changed; otherwise checked_add(unsuccessful_attempts_at_cursor, 1)
next_global = checked_add(consecutive_failures, 1)

if next_local >= max_attempts:
    finalize Failed and close RetryAttemptsExhausted
else if next_global >= MaxConsecutiveFailures:
    finalize Failed and close ConsecutiveFailures
else:
    persist suspension with next_local and next_global
```

Thus the local reason wins when both thresholds are reached by the same attempt. A state with `consecutive_failures >= MaxConsecutiveFailures` before ordinary admission is invalid and MUST be closed before another attempt. Productive close precedes the nonce target. System actors ignore User balance/fee gates.

`MinUserBalance` MUST satisfy `MinUserBalance >= AssetOps::minimum_balance(FeeNativeAsset)`. A stored nonce of `u64::MAX` is terminal for both classes: a lifecycle touchpoint that would install Active state or open another run performs pure close with `CycleNonceExhausted` before the requested effect. No nonce-exhausted paused state exists.

A lifecycle touchpoint is scheduler pop/admission, every actor-targeting control/tooling call, and successful address-event notification. Window bounds are inclusive (`start <= current_block <= end`); after `end`, closure precedes the requested mutation or ingress effect. Paired producer notification and originating movement remain one transaction; expired ingress is balance-only.

| State | Retained state |
| --- | --- |
| Dormant | identity, class and sovereign locator, mutability, owner, sovereign account, durable nonce, and User slot or occupied System locator |
| Active / Paused | identity plus complete program, funding accumulator, admission, readiness, scheduler, failure, and lease state |

Every creation path accepts `ProgramInput::Dormant` or `ProgramInput::Active(ActiveProgramInput)` subject to mutability rules. Activation accepts `ActiveProgramInput`. Active input contains schedule, optional window, plan, completion policy, funding policy, and optional auto-close target; it MUST validate all class, host-capability, trigger, observation, funding, Weight/fee, window, cardinality, namespace, and production-admission constraints before mutation. Dormant creation performs no plan scan or scheduler enrollment.

`CompletionPolicy::Persistent` leaves completed runs active. `CloseAfterProductiveRun` closes only after a completed logical run with `committed_effectful_tasks > 0`. A non-`StopCycle` task increments that counter only after committing its transaction.

Lifecycle transitions:

- `deactivate_aaa` cancels Continuation, invalidates readiness/membership, removes program, funding accumulator, subscriptions, and all active-epoch state, decrements `ActiveAaaCount`, and preserves identity, durable nonce, User slot or occupied System locator, and balances.
- `activate_aaa` validates and atomically installs a new Active epoch with an empty funding accumulator, incrementing `ActiveAaaCount`.
- Address events while Dormant are balance-only and do not become future `PercentageOfLastFunding` input.
- `ActorIdentityCount` counts active plus dormant identities; `ActiveAaaCount` counts active plus paused programs only.
- Any external control call that invalidates or reconstructs ordinary FIFO membership may do so at most once per actor per block; repetition fails with `QueueMutationRateLimited`. The rule applies to both classes and every origin. Signals still coalesce; internal scheduler transitions and terminal cleanup are exempt.

Terminal cleanup MUST prevalidate every fallible identity, cardinality, sovereign-registry, slot, reverse-index, subscription, funding, and exact-wakeup mutation. The committed cleanup then performs only actor-owned deletion and index repair, clears the live queue ticket as a tombstone, removes exact wakeup state, releases the User slot or marks the System locator `Vacant`, preserves balances, and emits one `AaaClosed`. It performs no task, fee, funding snapshot, compensation, shared scan, retry, or requeue.

Every bounded window owns direct terminal readiness at checked `end + 1` through `terminal_at`. One ordinary wakeup and one terminal-only requirement share one actor-local pointer: before queueing, the earlier target wins; after ordinary eligibility becomes a live queue ticket, a still-future terminal-only wakeup MAY coexist with that ticket. Two temporal wakeups may not coexist. `schedule_anchor` is reset on Active installation and schedule replacement; reactivation with `cycle_nonce > 0` uses it as the conservative cooldown anchor when no active-epoch `last_cycle_block` exists.

Explicit close executes pure cleanup inline; automatic close admits measured cleanup through the scheduler; sweep performs the same bounded cleanup without running a cycle. A pure close emits only `AaaClosed`; close after an open run follows the terminal `CycleSummary`. Custody recovery uses exact-slot User creation or `create_system_aaa_at_sovereign_id`; both create a fresh actor identity.

### 2.5 Continuation Cancellation and Invalidation

`cancel_continuation(aaa_id)` requires ordinary Mutable control and returns `ContinuationNotFound` when absent. Internal cancellation is idempotent. Cancellation performs no completion accounting, compensation, funding-delta restoration, prefix rollback, or balance movement.

A semantic change to execution plan/completion policy, funding policy, or schedule/window; deactivation; external close; window expiry; or incompatible runtime upgrade MUST cancel an open Continuation before changed meaning applies. Exact no-op updates MUST NOT cancel, reset clocks/failures, or emit update events. Auto-close target changes do not cancel.

Pause/resume and circuit-breaker transitions preserve Continuation. Cancellation invalidates membership actor-locally, emits `CycleCancelled` followed by `CycleSummary { result: Cancelled }`, and reconstructs one future path only when the actor remains active: retained `pending_signal` drives signalled policy; `Cadenced::Always` rearms from the resulting epoch. Deactivation and close suppress rearm.

Cancellation reasons are closed: explicit call -> `Explicit`; changed plan bytes -> `ExecutionPlanChanged`; completion-only change -> `CompletionPolicyChanged`; funding policy -> `FundingPolicyChanged`; schedule/window -> `ScheduleChanged`; deactivation -> `Deactivated`; lifecycle close -> `Closing(reason)`; incompatible upgrade -> `RuntimeUpgrade`.

After launch, every AAA-affecting runtime upgrade MUST ship one concrete non-consensus manifest:

```text
AaaUpgradeManifest {
    old_semantic_identity,
    new_semantic_identity,
    affected_surfaces,
    continuation_policy: Cancel | PreserveWithProof,
    evidence_identity,
}
```

`Cancel` is the default. `PreserveWithProof` is allowed only when the manifest proves equivalent step order, conditions, task/amount meaning, frozen trigger and funding inputs, failure classes, fee/Weight admission, and scheduler eligibility. Otherwise a bounded migration MUST cancel affected Continuations before execution resumes.

`StakingOps::share_asset(position_asset)` and ordered `LiquidityOps::lp_assets(lp_asset)` are admitted-plan identity bindings. They MUST NOT be reinterpreted in place; a changed binding requires a new position key or bounded revalidation/migration of every affected Active plan and cache.

### 2.6 Funding Delta

`PercentageOfLastFunding` resolves against authoritative funding accumulated since the previous logical-run opening within the current Active epoch. It does not mean the last individual transfer and does not depend on completion of the previous run. AAA exposes no dedicated funding extrinsic; accepted producer ingress is the only accumulator input.

1. Program admission scans every `PercentageOfLastFunding` resolution surface, including Unstake's `share_asset(position)`, builds the bounded tracked-asset set, and rejects missing bindings. Plan change atomically recomputes that set and deletes accumulated entries that are no longer tracked.
2. Each Active actor stores one explicit `FundingSourcePolicy`: `OwnerOnly`, `SignedAllowlist`, `RuntimePolicy`, or `AnyVerifiedIngress`. Immutable actors fix it at creation; Mutable control may replace it without rewriting accumulated values.
3. Ingress preserves two independent authenticated fields: optional concrete `source` and optional typed `provenance`. `OwnerOnly` and `SignedAllowlist` require Signed provenance plus, respectively, the matching owner or an allowlisted source. `AnyVerifiedIngress` accepts any authenticated ingress for which at least one field is verified; an all-`None` event remains balance-only. `RuntimePolicy` delegates both fields unchanged to default-deny `FundingAuthority`.
4. Trigger matching and funding authority are independent. Rejected or unclassified ingress remains valid balance movement but does not mutate funding state.
5. Every accepted positive tracked transfer checked-adds into `funding_accumulated[asset]`. Overflow returns `FundingAccumulatorOverflow` and rolls back both accumulator mutation and originating movement.
6. Before `CycleStarted`, runtime reads the complete bounded accumulator into a proposed funding snapshot, performs every remaining fallible attempt-admission check, then atomically increments `cycle_nonce`, commits `CycleStarted`, installs the transient snapshot, and clears the accumulated entries. No failed pre-opening admission may consume the accumulator.
7. The opening snapshot is the exact sum of accepted funding received after the previous logical-run opening and before the current opening. For the first run of an Active epoch, it is the accepted funding received since Active installation.
8. `PercentageOfLastFunding` reads only the frozen opening snapshot. Funding received after opening, including during suspension, accumulates for the next logical run and never changes the open run's basis.
9. A one-attempt run keeps the snapshot transient. Suspension persists only snapshot entries referenced by `cursor..plan.len()` in `ContinuationState.funding_snapshot`; retries reuse them while current capacity checks remain live.
10. Completion, failure, cancellation, pause, and breaker transitions neither restore the consumed snapshot nor alter the next accumulator. The delta boundary is run opening, not run outcome.
11. Deactivation and close delete the accumulator. Dormant, closed, and newly recreated actors do not infer authoritative funding from existing account balances or historical transfers.
12. `cycle_weight_upper` and `cycle_fee_upper` cache full-plan new-run bounds. Retry bounds use a bounded suffix scan. `funding_tracked_count` MUST exactly match the tracked set and update transactionally.

### 2.7 Logical-Run Accounting

- A run is `Completed` when the cursor reaches plan length or `StopCycle` terminates it. Failed `ContinueNextStep` outcomes do not prevent completion because the authored policy accepted those failures. `AbortCycle`, Permanent `RetryLater`, exhausted retry/failure bounds, and cancellation do not complete the run.
- Completion resets `consecutive_failures`, applies lease accounting, and evaluates completion-driven close. The summary's counters describe whether completion contained failures or skips; `Completed` does not assert that every task succeeded.
- An admitted attempt increments `consecutive_failures` exactly once when it suspends on Temporary/`FundingUnavailable` or terminates through `AbortCycle` or Permanent `RetryLater`. Deferral, cancellation, final skips, and `ContinueNextStep` failure do not independently increment it.
- `RetryLater.max_attempts` counts the initial unsuccessful attempt as `1`. Counter update and local/global precedence follow Section 2.4 exactly.
- Retry/failure exhaustion finalizes the run as `Failed`, emits one `CycleSummary`, and emits no `CycleCancelled`.
- Pre-admission RefTime/ProofSize deferral changes no nonce, attempt, clock, failure counter, funding accumulator, or run state.
- Durable `cycle_nonce` starts at `0`, checked-increments once at run opening, and is reused by retries. Deactivation preserves it. Close deletes the identity; any actor later attached to the same sovereign locator receives a fresh `aaa_id` and a fresh nonce sequence.
- `last_cycle_block` records opening only; retries do not change it. `last_attempt_block` records the latest admitted attempt. Retry admission checked-increments `attempt` before `CycleContinued`.
- Outcome totals count emitted outcomes across attempts; `committed_effectful_tasks` counts only committed economic tasks.
- Before `CycleStarted` or `CycleContinued`, complete-attempt admission MUST preflight every reachable fallible consequence: retry/rearm placement, possible cleanup, funding-snapshot consumption, and bounded producer-ingress namespace demand. After the first task effect commits, finalization MUST contain no fallible transition.
- Opening a signalled run consumes only the latch present at opening. Later signals remain latched. Terminal finalization reconstructs exactly one future path for retained readiness or `Cadenced::Always`; suspension owns the retry path and reserves the latch for the next run.
- Completed finalization resets failures, emits `CycleSummary { result: Completed }`, then evaluates productive close before auto-close. Failed and cancelled runs emit their corresponding result. `AaaClosed`, when applicable, follows the summary.

## 3. Adapters and Host Services

All external state operations MUST use typed adapters or host services. Mutation atomicity follows Section 5.5; Weight ownership follows Section 3.6.

### 3.0 Typed Failure Contract

Mutation adapters return `TaskFailure { error: DispatchError, retry: RetryClass }`, where `RetryClass` is `Permanent | Temporary`. Retryability MUST NOT be inferred from strings, module indices, broad token errors, or raw `DispatchError` coincidence. Unknown and unconfigured mutation failures default to Permanent; diagnostics remain event-local and never enter Continuation.

Each binding MUST expose a deterministic bounded admission validator for statically knowable capability, asset, pair, position, and parameter restrictions. Active-plan admission rejects unbound or statically unsupported requirements; dynamic balance, route, liquidity, and pool-state failures remain execution-time. An upgrade MUST revalidate before removing an admitted static capability.

Adapter success has effect fidelity and debit confinement: a positive successful request MUST commit the declared effect, debit only task-declared source surfaces, remain within AAA-supplied bounds, and consume no hidden sovereign deposits, taxes, auxiliary assets, or `FeeNativeAsset` capacity.

| Failure surface | Disposition |
| --- | --- |
| Condition/amount error; fee collection failure | Permanent; apply policy, never suspend |
| `FundingUnavailable` | advance under ordinary policies; suspend only under `RetryLater` |
| Current slippage/cap/reference/liquidity/output/ratio movement | Temporary |
| Unsupported/identical assets, malformed or zero request, forbidden capability, invalid identity, arithmetic/configuration/authorization failure | Permanent |
| Other mutation failure | adapter-supplied class; unknown defaults Permanent |

### 3.1 AssetOps

```rust
trait AssetOps<AccountId, AssetId, Balance> {
    fn transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
    fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
    fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
    fn balance(who: &AccountId, asset: AssetId) -> Balance;
    fn minimum_balance(asset: AssetId) -> Balance;
    fn preflight_transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
}
```

`balance()` returns immediately transferable adapter-visible balance before AAA-local reservation. AAA subtracts `reserved_fee_remaining` only for `FeeNativeAsset`. `Mint` is rejected for User plans at every admission path. `preflight_transfer` MUST model the following transfer's actual withdrawal/deposit consequences under unchanged state and classify failure explicitly. Unit fallback fails every mutation/preflight Permanently.

### 3.2 DexOps

```rust
struct ExecutionContext<'a, AccountId> {
    actor: &'a AccountId,
    aaa_type: AaaType,
}

trait DexOps<AccountId, AssetId, Balance> {
    fn swap_exact_in(
        context: ExecutionContext<'_, AccountId>,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_in: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, TaskFailure>;

    fn swap_exact_out(
        context: ExecutionContext<'_, AccountId>,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_out: Balance,
        max_amount_in: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, TaskFailure>;
}
```

1. Methods are deterministic O(1) or bounded O(K), use canonical iteration, and fix rounding. `AaaType` is projected from stored `ActorClass`; adapters MUST NOT infer it from account shape or catalogs.
2. `SwapIn` derives `min_out` from the same caller-aware executable quote/routing mechanism used for execution. Success debits exactly `amount_in` and credits/returns strictly positive `amount_out >= min_out`.
3. `SwapOut` derives a caller-aware required-input quote, applies tolerance, and requires it within the finite effective capacity supplied by AAA. Success credits exactly `amount_out` and debits/returns strictly positive `amount_in <= max_amount_in`.
4. AAA supplies resolved task-local amounts; it applies no generic System raw-balance cap. `SwapOut` alone receives current preservable input capacity, optionally limited by `InputLimit::Absolute`.
5. Before mutation, every System swap compares directed execution ratio (`asset_out / asset_in`) with a nonzero reference. A fresh nonzero EMA is preferred; otherwise the nonzero direct-pair reserve ratio is used. Acceptance is:

   `abs(exec_out * ref_in - ref_out * exec_in) * Perbill::ACCURACY <= SystemSwapMaxReferenceDeviation.deconstruct() * ref_out * exec_in`

   Products use checked sufficient width. Missing/exceeded reference is Temporary; arithmetic-width misconfiguration is Permanent.
6. The reference-deviation guard is local execution protection, not a fair-price or transaction-ordering guarantee. Slippage and exact-output limits remain independently authoritative.

### 3.3 StakingOps

```rust
trait StakingOps<AccountId, AssetId, Balance> {
    fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
    fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), TaskFailure>;
    fn share_balance(who: &AccountId, asset: AssetId) -> Balance;
    fn share_asset(asset: AssetId) -> Option<AssetId>;
}
```

AAA encodes no collator, nomination, receipt, or native-staking topology. `Unstake.asset` is a runtime position key; current/trigger/all modes use `share_balance`, and last-funding uses the transferable `share_asset` batch. Missing mapping fails admission or execution Permanently. Position-to-share binding is stable under Section 2.5.

### 3.4 LiquidityOps

```rust
trait LiquidityOps<AccountId, AssetId, Balance> {
    fn add_liquidity(
        who: &AccountId,
        asset_a: AssetId,
        asset_b: AssetId,
        amount_a: Balance,
        amount_b: Balance,
        min_lp_out: Balance,
    ) -> Result<(Balance, Balance, Balance), TaskFailure>;

    fn lp_assets(lp_asset: AssetId) -> Option<(AssetId, AssetId)>;

    fn remove_liquidity(
        who: &AccountId,
        lp_asset: AssetId,
        asset_a: AssetId,
        asset_b: AssetId,
        lp_amount: Balance,
        min_amount_a: Balance,
        min_amount_b: Balance,
    ) -> Result<(Balance, Balance), TaskFailure>;

    fn donate_liquidity(
        who: &AccountId,
        asset_a: AssetId,
        asset_b: AssetId,
        max_amount_a: Balance,
        max_amount_b: Balance,
        max_ratio_error: Perbill,
    ) -> Result<(Balance, Balance), TaskFailure>;
}
```

All methods are deterministic O(1) or bounded O(K) and task-atomic.

- `AddLiquidity` treats resolved amounts as debit caps; success returns `0 < used_a <= amount_a`, `0 < used_b <= amount_b`, and `lp_out >= min_lp_out`, and emits those exact values.
- `RemoveLiquidity` carries the expected ordered pair. Admission and execution require `lp_assets(lp_asset) == Some((asset_a, asset_b))`; success burns exactly `lp_amount` and returns outputs meeting both minima.
- `DonateLiquidity` receives resolved `max_amount_a` and current preservable `max_amount_b`; success debits and returns positive amounts within both caps. Market ratio/cap movement is Temporary; malformed pair semantics are Permanent.
- Pool creation, LP accounting, ratio policy, and native routing belong to the adapter, but MUST NOT introduce a third sovereign debit surface. Unit fallback fails Permanently.

### 3.5 Runtime Host Services

```rust
trait FeeCollector<AccountId, AssetId, Balance> {
    fn collect_fee(
        payer: &AccountId,
        fee_sink: &AccountId,
        native_asset: AssetId,
        amount: Balance,
    ) -> DispatchResult;
}

trait ObservationProvider<FeedId, BlockNumber> {
    fn observe(feed: &FeedId, now: BlockNumber, max_age_blocks: u32) -> Observation<BlockNumber>;
}

trait FundingAuthority<AaaId, AccountId, Provenance> {
    fn permits(
        aaa_id: AaaId,
        owner: &AccountId,
        source: Option<&AccountId>,
        provenance: Option<&Provenance>,
    ) -> bool;
}

trait SovereignAccountPolicy<AccountId> {
    fn is_reserved(account: &AccountId) -> bool;
}
```

Host services are deterministic, bounded, and covered by the caller's generated Weight. Missing mutation or fee capability fails closed; missing observation returns `Unavailable`; missing funding authority denies. `SovereignAccountPolicy` is always explicit.

`FeeCollector` is one explicit certified producer to the configured `FeeSink`: it atomically moves exactly the configured fee-native amount into deposit-capable reserved `FeeSink`, performs no split, and submits exactly one paired AddressEvent (payer source, typed internal-protocol provenance) in the same transaction so the Fee Sink actor latches readiness for its bounded plan. It MUST NOT recursively charge an AAA fee, invoke unrelated asset-adapter special cases, duplicate notification, or create an event for zero/no-op movement. A host Fee Sink MAY latch readiness while its default-deny `RuntimePolicy` accumulates no authoritative funding.

`ObservationProvider` returns `Fresh { value, observed_at }`, `Stale`, `Uninitialized`, or `Unavailable`. `Fresh` is valid only when `observed_at <= now` and `now - observed_at <= max_age_blocks`. An invalid `Fresh` result is a Permanent condition-evaluation failure, not `false`, `Stale`, or `Unavailable`: after successful collection of the applicable evaluation fee, runtime emits `StepFailed { retry_class: Permanent }`, applies `StepErrorPolicy`, and dispatches no task. If fee collection itself fails, Section 4.1 owns that failure instead.

`FundingAuthority` is used only by `RuntimePolicy`, receives source and provenance without inference, and denies the all-`None` case. `SovereignAccountPolicy` covers every runtime-controlled account domain; policy changes MUST resolve conflicts with active/dormant identities and every allocated System sovereign locator, whether occupied or vacant, before taking effect.

Address-event ingress uses one paired contract over `AddressEvent`:

1. `preflight_address_event(event)` runs before value movement, performs no mutation, and detects every fallible lifecycle, funding, trigger, and placement consequence.
2. `notify_address_event(event)` runs exactly once after successful movement in the same outer transaction. Failure rolls back notification and movement.
3. Absent/dormant destinations and zero amount are balance-only. For Active actors, funding authority and trigger matching are independent.
4. Every adapter path that credits an AAA sovereign account is a producer. Self/no-op movement manufactures no ingress. Fee collection is covered ONLY by the single certified `FeeCollector` producer (Section 3.5): the fee-native ledger movement itself is NOT a generic transfer/transaction-extension producer, and no other path may notify the same movement a second time.
5. Generic transaction-extension coverage is allowed only with Executive-level proof of rollback and post-dispatch Weight correction. Runtime event scanning is not ingress.
6. Embedding evidence MUST include a complete crediting-producer inventory naming every producer path, credited surface, source/provenance semantics, preflight owner, notification owner, rollback witness, Weight owner, or explicit exclusion reason.

### 3.6 Weight Contract

```rust
fn task_execution_weight_upper_bound(aaa_type: AaaType, task: &Task) -> Weight;
fn step_admission_weight_upper_bound(aaa_type: AaaType, step: &Step) -> Weight;
fn cycle_admission_weight_upper_bound(aaa_type: AaaType, steps: &[Step]) -> Weight;
```

- Task bound covers successful task dispatch, adapter algorithms, producer ingress, rollback framing, and task success event.
- Step bound covers all condition reads without short-circuit, amount preparation, at most one User fee collection, task execution where applicable, rollback, and the largest non-task step event.
- Cycle bound adds run opening, funding-snapshot consumption, finalization, and cumulative accounting. Pure cleanup owns an independent generated bound.
- Bounds are state-independent for fixed encoded parameters, deterministic, O(1) or bounded O(K), and component-wise no lower than actual RefTime/ProofSize. Checked composition overflow returns `AdmissionBoundOverflow`.
- Program admission requires one actor probe plus the complete run or unresolved suffix plus possible cleanup to fit the global actor-service envelope after fixed worker envelopes. Class changes fee and adapter branches, not scheduler priority or plan length.
- Weight, page-size, worker-limit, or adapter-bound changes require accepted production evidence and bounded cache recomputation or revalidation before affected execution resumes.
- A new core `Task` requires bounded types, resolution/funding semantics, adapter ownership, policy, events/errors, rollback, generated Weight, admission, tests, and explicit SCALE impact. Product topology belongs in adapters or actor graphs.

Distinct execution buckets SHOULD remain separate where proof or algorithm bounds differ: `TransferIngress`, `Burn`, `MintIngress`, `DexSwapIn`, `DexSwapOut`, `AddLiquidity`, `RemoveLiquidity`, `DonateLiquidity`, `Stake`, `Unstake`, `SplitTransfer(legs)`, and adapter-free `StopCycle`.

## 4. Economics

System AAA pays no User fee. This exemption is a governance trust boundary, not a protocol-level economic throttle. Every User fee passes through one `FeeCollector` transaction into `FeeSink`; AAA performs no downstream split. Terminal cleanup charges no AAA fee.

### 4.1 Fee Model

User attempt order:

1. `MinUserBalance` gate.
2. Full-plan or unresolved-suffix fee admission.
3. Logical-run opening and transient reservation.
4. For each suffix step: evaluate all conditions and prepare inputs read-only; choose evaluation-only or combined fee; collect at most once; release the step envelope; dispatch only after successful collection.

After the balance gate:

`fee_native_balance = AssetOps::balance(sovereign_account, FeeNativeAsset)`

`available_fee_budget = fee_native_balance.checked_sub(MinUserBalance).unwrap_or(0)`

If `available_fee_budget < admission_fee_upper(cursor)`, the User actor closes with `FeeBudgetExhausted`. New runs cover the full plan; retries cover `cursor..plan.len()`. Committed-prefix fees and Weight do not recur.

Per-step formulas:

- `condition_count(Always) = 0`
- `condition_count(All(v) | Any(v)) = v.len()`
- `eval_fee = StepBaseFee + ConditionReadFee * condition_count`
- `exec_fee_upper = WeightToFee(task_execution_weight_upper_bound(User, task))`
- `cycle_fee_upper = checked_sum(eval_fee + exec_fee_upper)`

Any conversion or composition overflow returns `AdmissionBoundOverflow` during program admission.

One package-owned pure derivation, `attempt_fee_envelope(plan, cursor, aaa_type)`, MUST be the sole runtime owner of per-step and total fee envelopes. Program admission, transient reservation, execution, simulation, and benchmark fixtures MUST call that derivation directly. Metadata-bound client tooling MUST consume generated conformance vectors rather than maintain an independent fee formula. Property tests MUST prove that the total equals the checked sum of the suffix envelopes and that releasing each attempted step reaches exactly zero without violating the User protected floor.

| Read-only result | Collected fee | Result |
| --- | --- | --- |
| condition false | `eval_fee` | `StepSkipped(ConditionsNotMet)`; advance |
| condition evaluation error | `eval_fee` | `StepFailed(Permanent)`; apply policy; no task |
| resolution `Skipped` | `eval_fee` | `StepSkipped(ResolutionSkipped)`; advance |
| resolution error | `eval_fee` | `StepFailed(Permanent)`; apply policy; no task |
| `FundingUnavailable` | `eval_fee` | final advancing skip, or suspension only under valid `RetryLater` |
| executable | `eval_fee + exec_fee_upper` | dispatch after collection |
| collection failure | selected amount attempted once; debit rolls back | `StepFailed(Permanent)`; apply policy; no task |
| adapter failure | combined fee remains charged; task effects roll back | `StepFailed(adapter class)`; apply policy |

A failed `FeeCollector` call commits no fee debit, dispatches no task, emits `StepFailed { retry_class: Permanent }`, and follows the configured `StepErrorPolicy`. It cannot cause a second collection attempt. `StepBaseFee` and `ConditionReadFee` MUST cover non-executable read/write/event paths. `StepErrorPolicy` applies only after fee determination.

### 4.2 No Rent

AAA accrues no recurring rent or touch-based maintenance debit. Deferred actors remain valid within lifecycle and admission bounds.

### 4.3 Fee Reservation

During an admitted User attempt:

`spendable_fee_native = max(fee_native_balance - reserved_fee_remaining, 0)`

`user_fee_native_floor = max(MinUserBalance, AssetOps::minimum_balance(FeeNativeAsset))`

`reserved_fee_remaining` is transient and never stored.

1. Initialize it to the admitted full-plan or suffix fee bound.
2. Read-only preparation observes the current and later step envelopes as reserved. After the selected collection succeeds or fails, remove the current step's full envelope before later-step resolution.
3. Every User `FeeNativeAsset` debit-cap calculation uses Section 5.3 with `user_fee_native_floor`, including direct task amounts, `SwapOut` input capacity, and the auxiliary `asset_b` donation cap. No successful User task may leave the fee-native balance below `MinUserBalance` after all reserved fees for the attempt are charged.
4. Attempt exit discards unused reserve; collected fees are not refunded.
5. AAA charges deterministic upper-bound task fees and performs no actual-weight refund. FRAME post-dispatch Weight correction may adjust block accounting but MUST NOT refund or reinterpret the AAA attempt fee.

### 4.4 Opening Fee

`create_user_aaa` and `create_user_aaa_at_slot` charge non-refundable `AaaCreationFee` through `FeeCollector`. Failure to cover it plus normal transaction fees returns `InsufficientFee` and rolls back creation. Both fresh and locator-reuse System creation paths are exempt.

### 4.5 Terminal Cleanup Admission

Pure cleanup has an independently generated class/branch-sensitive two-dimensional bound. Scheduler cleanup defers without mutation when either Weight dimension cannot fit; explicit calls declare the conservative bound before execution.

---
## 5. Execution

### 5.1 Linear Control

```rust
struct Step {
    conditions: ConditionSet<Condition, MaxConditionsPerStep>,
    task: Task,
    on_error: StepErrorPolicy,
}
```

Each step has one fixed successor, `i + 1`, or terminal. Current state may decide whether its task executes and earlier committed tasks may change later reads, but no surface may rewrite the program or select an arbitrary successor.

| Current outcome | Control |
| --- | --- |
| condition false | `i + 1` |
| resolution `Skipped` | `i + 1` |
| economic task success | `i + 1` |
| `StopCycle` success | completed terminal |
| failure + `ContinueNextStep` | `i + 1` |
| failure + `AbortCycle` | failed terminal |
| Temporary failure + valid `RetryLater` | remain at `i` |
| Permanent failure + `RetryLater` | failed terminal |
| `FundingUnavailable` | advance under ordinary policies; remain at `i` only under valid `RetryLater` |

Within one run, `cursor' ∈ { cursor, cursor + 1, terminal }` and never decreases. Conditions can only execute/skip the current task; amount resolution can only return a closed resolution outcome; task output cannot select control; adapter branching remains internal to one bounded atomic task; actor-to-actor feedback remains scheduler-mediated and subject to the block cutoff.

### 5.2 Conditions

`Always` has no atoms and is true. `All(v)` and `Any(v)` require `1..=MaxConditionsPerStep`; empty aggregates fail with `EmptyConditionSet`. Nesting is impossible because `Condition` is atomic.

Production and rollback-only simulation MUST use one evaluator, read every atom without short-circuit, and aggregate only after all reads. Any atomic evaluation error makes the aggregate a Permanent condition failure, even when another `Any` atom is true. False emits `StepSkipped(ConditionsNotMet)` and advances.

Balance conditions read spendable balance, including the current fee reservation. Observation conditions use exact `u128` comparison over valid `ObservationProvider::Fresh`; `Stale`, `Uninitialized`, and `Unavailable` are false, while an invalid `Fresh` is the Permanent evaluation failure defined in Section 3.5. `max_age_blocks` MUST be nonzero. Conditions are pure and cannot write state, select tasks, or create Continuation.

### 5.3 Amount Resolution

Internal resolution is exhaustive:

```rust
enum ResolutionOutcome<Balance> {
    Resolved(Balance),
    Skipped,
    FundingUnavailable,
}
```

Percentage modes use checked widened floor division:

`floor(base * perbill / Perbill::ACCURACY)`

The protected source floor is:

```text
protected_minimum(class, asset) =
    max(MinUserBalance, AssetOps::minimum_balance(asset))
        when class is User and asset == FeeNativeAsset;
    AssetOps::minimum_balance(asset)
        otherwise
```

For a User `FeeNativeAsset` surface, `spendable_current` already excludes `reserved_fee_remaining` under Section 4.3.

Policies:

| Policy | Tasks | Semantics |
| --- | --- | --- |
| `PreserveSource` | Transfer, SplitTransfer, SwapIn, AddLiquidity, RemoveLiquidity, Burn, Stake, DonateLiquidity | `preservable_current = max(spendable_current - protected_minimum(class, asset), 0)`; current percentages use it; fixed/snapshot amounts must fit it; `AllBalance` equals it |
| `OutputTarget` | Mint, SwapOut | target amount does not subtract a source minimum; SwapOut separately receives finite preservable input capacity computed with the same protected source floor |
| `ShareSpend` | Unstake | uses adapter-visible shares and permits full withdrawal |

`PercentageOfTrigger` uses the frozen trigger snapshot, then current capacity. `PercentageOfLastFunding` uses the frozen funding-delta snapshot taken at logical-run opening under Section 2.6, then current capacity. Under `OutputTarget`, `AllBalance` resolves to the full current target-asset balance surface. Funding received after opening never changes the current run.

`Skipped` means a valid dynamic intent resolves to zero. `FundingUnavailable` means a required funding-snapshot entry is absent or zero, a positive directly resolved debit exceeds known current source/share capacity, or a required auxiliary debit cap is zero. Market demand above a nonzero finite SwapOut/donation cap is a Temporary adapter failure, not `FundingUnavailable`.

Resolution MUST NOT clamp. Multi-surface tasks resolve every field/cap before dispatch with precedence `FundingUnavailable > Skipped > Executable`. Static validation rejects `Fixed(0)`, zero percentages, `InputLimit::Absolute(0)`, unsupported amount modes, and zero liquidity minima. Dynamic zero remains `Skipped`.

For `SwapOut`, AAA resolves the target under `OutputTarget`, computes current preservable `asset_in` capacity with the protected source floor, and applies `Absolute` as an additional ceiling. `DonateLiquidity` computes its auxiliary `asset_b` cap through the same rule. `AddLiquidity` requires fixed `min_lp_out > 0`; `RemoveLiquidity` requires both fixed minima > 0. Bound failure rolls back the task.

### 5.4 Trigger Snapshot

`PercentageOfTrigger` uses one cycle-start balance snapshot, never an event payload.

1. During read-only opening preparation, after transient fee reservation but before `CycleStarted`, scan unique trigger-resolution surfaces.
2. Ordinary assets snapshot `spendable_fee_native` or `AssetOps::balance`; Unstake snapshots `StakingOps::share_balance` under `ResolutionSurface::StakingShares`.
3. One-attempt runs persist nothing. Suspension stores only surfaces referenced by the unresolved suffix.
4. Plan admission MUST fit the unique-surface count within `MaxContinuationSnapshotEntries`, which MUST be `<= 2 * MaxExecutionPlanSteps`.
5. Retries reuse frozen values while current minimum/spendability checks remain live. Missing required snapshot is a Permanent invariant failure.
6. Any plan using `PercentageOfTrigger` requires a nonempty source set containing only `OnAddressEvent`; Manual, observation, `Cadenced::Always`, and mixed source families are invalid.
7. Every AddressEvent asset filter MUST cover every required ordinary/share asset; missing share mapping fails admission.
8. Coalesced events do not retain sender, event amount, or event list; the snapshot is the exact opening balance per typed surface.

### 5.5 Error Policies and Atomicity

```rust
enum StepErrorPolicy {
    AbortCycle,
    ContinueNextStep,
    RetryLater { max_attempts: u32 },
}
```

- `ContinueNextStep`: roll back the task, record final failure, and advance in the same attempt. Reaching terminal after such failures still yields `CycleResult::Completed`.
- `AbortCycle`: roll back the task and terminate the run as `Failed`; the next signal starts at step `0`.
- `RetryLater` + Temporary: roll back, retain the committed prefix, apply the exact local/global threshold algorithm in Section 2.4, and when both thresholds remain below their bounds persist Continuation at the unresolved cursor without another trigger.
- `RetryLater` + Permanent: roll back and terminate as `Failed` without retry state.
- `FundingUnavailable`: final advancing skip except under `RetryLater`, where it follows the same bounded suspension contract.

Every executable task runs in one task-scoped transaction. Multi-operation tasks share that boundary. Failure reverts every task-local adapter, transfer, producer-ingress, and success-event effect while preserving earlier committed steps. The collected User fee and outer `StepFailed` event remain outside the task rollback. Whole-plan rollback, task-authored checkpoints, and actor-wide pause policies are forbidden.

### 5.6 Sparse Continuation

At suspension, one scalar `cursor` proves: every lower step has a final outcome, the cursor step remains unresolved, no later step executed, and retry resumes at that cursor. No completion bitmap exists.

`ContinuationState` exists only while suspended, is keyed by `aaa_id`, and MUST NOT duplicate durable nonce, identity, owner, account, plan, prepared tasks, full balances, or program hash. It persists only unresolved-suffix trigger and funding snapshots. Suffix Weight/fee bounds derive from a bounded scan beginning at cursor.

`ActorHot.run_state` is the sole discovery marker: `Idle` causes no Continuation read; `Suspended` requires exactly one value. The invariant

`ActorHot.run_state == Suspended <=> ContinuationState exists`

MUST hold through suspension, cancellation, invalidation, close, migration, and try-state.

## 6. Tasks

### 6.1 Task Contract

- `Transfer`: one asset transfer.
- `SplitTransfer`: bounded atomic fan-out.
- `Burn`: asset burn.
- `Mint`: System-only asset mint.
- `SwapIn`: exact-input swap with attempt-time quote and tolerance.
- `SwapOut`: exact-output swap with `InputLimit::{LiveQuote, Absolute}` and tolerance.
- `AddLiquidity`: bounded two-asset provision with nonzero LP minimum.
- `RemoveLiquidity`: bounded withdrawal of an explicitly bound ordered LP pair with two nonzero minima.
- `Stake`: stake the declared asset through `StakingOps`.
- `DonateLiquidity`: donate within finite `asset_a` and current `asset_b` caps without LP issuance.
- `Unstake`: withdraw shares from a runtime position key.
- `StopCycle`: fieldless completed termination of the current run.

`SwapIn` computes `min_out = floor((1 - tolerance) * executable_quote_output)`. Zero tolerance accepts no deterioration; full tolerance still requires positive output.

For `SwapOut`, AAA supplies finite effective input capacity. Define exact integer rounding in a widened domain:

```text
ceil_perbill(x, p) =
    (wide(x) * p.deconstruct() + Perbill::ACCURACY - 1)
    / Perbill::ACCURACY

quoted_max_in = checked_add(
    quote_required_in,
    checked_narrow(ceil_perbill(quote_required_in, tolerance))
)
```

Every multiplication and addition is checked in the widened domain; narrowing to `Balance` is checked. A result above `Balance` range or effective cap fails Temporary without debit; inability of the configured implementation width to represent the specified widened calculation is Permanent. `LiveQuote` accepts pre-attempt price movement within preservable balance; `Absolute` also enforces the authored ceiling. Exact-input quotes MUST NOT be inverted across the balance width.

Every Active-program admission path rejects self `Transfer`, self `SplitTransfer` leg, identical swap assets, identical liquidity/donation assets, mismatched `RemoveLiquidity` pair, forbidden zero values, unsupported capabilities, and class-forbidden tasks. Execution rechecks adapter-owned identities that may change.

### 6.2 SplitTransfer

Validation:

1. `2 <= legs.len() <= MaxSplitTransferLegs`.
2. Every share is positive.
3. Recipients are unique and differ from the sovereign account.
4. `sum(shares) <= Perbill::one()`.

Allocation uses Section 5.3 floor rounding:

- `leg_i = floor(total * share_i)`
- `distributed = sum(leg_i)`
- `retained = total - distributed`

If `distributed == 0`, resolution is `Skipped`. An undeclared share and integer dust remain on the sovereign account; rejected legs are not remainder.

Runtime computes all legs in declared order, preflights every nonzero leg, then executes the whole fan-out atomically. Recipient deposit ineligibility is typed Temporary `RecipientDepositUnavailable`; any failure commits no transfer, success event, or retained-accounting effect.

### 6.3 StopCycle

After true conditions and successful User fee collection, `StopCycle` records execution, emits `CycleStopped`, skips the suffix, and finalizes as `Completed`. Finalization clears Continuation, resets failures, emits the cumulative completed summary, then applies productive-close and auto-close precedence.

The task body has no parameters, amount, adapter, economic effect, retry state, lifecycle mutation, or scheduler mutation. It increments `executed_steps`, not `committed_effectful_tasks`. A false condition skips normally. Pre-execution failures obey `on_error`; therefore `StopCycle + ContinueNextStep` may fall through to later steps.

### 6.4 Exhaustive Instruction Contract

The package MUST derive one machine-readable classification for every `Task`, `Condition`, `AmountResolution`, and `StepErrorPolicy` from exhaustive Rust matches. Adding a variant MUST fail compilation until classified; no handwritten primitive registry may own semantics.

The classification exposes adapter ownership, assets/recipients/effects, class availability, committed-effect possibility, fixed successful control, bounded algorithm and Weight owner, condition reads/purity, amount dependencies/checks, and error-policy controls. It adds no storage, runtime API, or independent Weight model. Tasks MUST NOT dispatch arbitrary extrinsics.

---

## 7. Triggers

### 7.1 Trigger Policy

```rust
enum TriggerSource<AccountId, AssetId, ObservationFeedId> {
    Manual,
    OnAddressEvent {
        source_filter: SourceFilter<AccountId>,
        asset_filter: AssetFilter<AssetId>,
    },
    OnObservationChange { feed: ObservationFeedId },
}

enum CadenceMode<Sources> {
    Always,
    WhenSignalled(Sources),
}

enum TriggerPolicy<Sources> {
    Immediate { sources: Sources },
    Cadenced { every_blocks: u32, mode: CadenceMode<Sources> },
}
```

`Immediate` and `Cadenced::WhenSignalled` require a nonempty canonical bounded source set; `Cadenced::Always` has none. Source sets and all whitelists/allowlists are duplicate-free and strictly ordered by canonical SCALE bytes. Sources compose as OR and are fully evaluated without short-circuit.

| Policy | Admission |
| --- | --- |
| `Immediate` | requests placement when `pending_signal` is set |
| `Cadenced::WhenSignalled` | latches immediately and admits no earlier than cadence |
| `Cadenced::Always` | admits on cadence without a latch and rearms after every terminal run while active |

A trigger can only change readiness; it cannot select actor class, task, parameter, adapter, or successor, and cannot bypass timing, cutoff, Weight, fee, or FIFO gates.

Timing:

1. `cooldown_blocks` applies after the lifetime-first admitted run. The exemption exists only when `cycle_nonce == 0 && last_cycle_block == None`.
2. Otherwise `cooldown_anchor = last_cycle_block.or(schedule_anchor)`; new-run eligibility is the maximum of cooldown, cadence, and window start.
3. Retry backoff is protocol-fixed, not a runtime binding or actor parameter: persisted attempts `0, 1, 2, >=3` map to `1, 2, 4, 8` blocks. Retry eligibility is `last_attempt_block + max(cooldown_blocks, protocol_backoff)`, also bounded by window start. External cadence does not gate an open run.
4. `every_blocks > 0`; `every_blocks`, `cooldown_blocks`, and maximum jittered delay MUST fit `MaxExecutionDelayBlocks`.
5. Deterministic jitter for `every_blocks > 1` is:

   `jitter_window = min(every_blocks / 4, MaxTimerJitterBlocks)`

   `jitter = u64::from_le_bytes(Blake2_256(SCALE(aaa_id))[0..8]) % jitter_window`, or `0` when the window is zero.

   `cadence_eligible_at = max(schedule_anchor, last_cycle_block.unwrap_or(schedule_anchor)) + every_blocks + jitter`.

   Jitter is anti-clustering only. It provides no secrecy, adversarial unpredictability, anti-targeting property, transaction-ordering protection, or MEV protection.
6. All delay composition is saturating only where explicitly stated; placement follows the single queue/wakeup contract in Section 8.1.

### 7.2 Trigger Sources and Ingress

```rust
enum SourceFilter<AccountId> {
    Any,
    OwnerOnly,
    Whitelist(BoundedVec<AccountId, MaxWhitelistSize>),
}

enum AssetFilter<AssetId> {
    Any,
    Whitelist(BoundedVec<AssetId, MaxWhitelistSize>),
}
```

`ActorHot.pending_signal` is the sole readiness latch for all sources. A match changes it only `false -> true`; multiple matches coalesce. Deferral and pause preserve it; an admitted signalled run clears only the latch present at opening; later signals remain set. Cleanup removes it.

The source model has the following exact consequences:

- ten transfers do not guarantee ten logical runs;
- ten observation revisions do not guarantee ten attempts;
- a signal received during an open Continuation does not alter that run;
- such a signal may prepare the next run;
- conditions read latest state at attempt time, not state captured when the signal arrived.

Source validation rejects duplicate Manual atoms, duplicate AddressEvent filter pairs, duplicate observation feeds, empty whitelists, duplicate members, and noncanonical ordering. Events without a concrete source match only `SourceFilter::Any`. Observation sources carry latest-state reconsideration only; conditions read the observation later.

Ingress rules:

1. Observation changes use bounded dirty state and deferred fanout; publication never scans subscribers or executes actors.
2. Every AddressEvent producer uses paired `preflight_address_event` / `notify_address_event` under Section 3.5. Producers do not mutate readiness or funding directly.
3. Concrete sender and typed provenance remain independent. Source-less verified provenance remains distinguishable from wholly unclassified movement.
4. Every successful producer call is applied exactly once; equal-content transfers remain distinct for funding accumulation even while the latch coalesces.
5. Funding authority and trigger matching remain independent. Producer paths pay their own mutation Weight.
6. Deferred event-vector scanning, per-event inboxes, revision queues, and compatibility ingress storage are forbidden.

### 7.3 Observation Subscription Index

Subscriptions derive only from canonical `OnObservationChange { feed }` source atoms. Each Active actor stores its duplicate-free feeds in source order; plan conditions do not create subscriptions.

1. An actor with observation sources owns one reusable dense slot below `MaxActiveActors`; others own none.
2. Free slots use an `ObservationPageSize`-bounded paged LIFO with exact forward/reverse ownership.
3. Subscriber cell key is `(feed, slot / ObservationPageSize)` with in-page index `slot % ObservationPageSize`. Each feed links only occupied pages through exact doubly linked topology.
4. Empty pages and zero feed counts are deleted immediately. Fanout traverses occupied pages only. Global subscriptions are bounded by `MaxActiveActors * MaxTriggerSources`.
5. Creation/activation install subscriptions atomically; schedule replacement applies exact diff; plan-only replacement changes none; deactivation/close removes exact entries and slot. Removing the final subscriber deletes the feed revision baseline and dirty state.
6. Membership is live: removal before visit cancels delivery; addition during a pass may receive it if encountered, but only a later changed publication is guaranteed.
7. Try-state reconciles actor/feed/slot/page/count/revision topology.

### 7.4 Dirty Observation Ingress

A changed observation publication MUST call `note_observation_changed(feed, revision)` in the same transaction.

For continuously subscribed feeds, `ObservationIngressRevisions[feed]` stores the highest accepted revision. Revision `0` or regression fails with `InvalidObservationRevision`; equality is an idempotent no-op; a greater revision updates the baseline and creates/updates one dirty obligation. Unsubscribed feeds allocate nothing.

Dirty state stores `latest_revision`, `fanout_revision`, `dirty_since`, optional exact next page, and reciprocal active-list links. First insertion sets `fanout_revision = 0`, records current block, and has no page cursor. Newer revisions update `latest_revision` without resetting `dirty_since`; clean completion deletes dirty state but retains the baseline. Dirty age is `finalized_block.saturating_sub(dirty_since)`.

Ingress is O(1): it MUST NOT read subscriber pages, mutate actors, enqueue, evaluate conditions, or execute. The active-dirty list has exact head/tail/fair cursor/count, bounded by live subscribed feeds. Mutation is transactional and reports one conservative worst-branch Weight independent of subscriber count.

### 7.5 Deferred Observation Fanout

Fanout runs in `on_idle` after due wakeups and before cutoff/actor execution. It is bounded by `ObservationFanoutWeightLimit` and `MaxObservationFanoutPagesPerBlock`; neither value creates a second scheduler or a per-class service right.

Each admitted unit touches at most one occupied page and reserves complete two-dimensional Weight before mutation. It selects the fair-cursor feed, rotates fairly, sets `pending_signal` for every live subscriber, and creates or preserves one canonical admission path. A page advances only when every live entry has such a path; queue saturation may leave partial latches while the same page retries.

The first page of a pass snapshots `fanout_revision = latest_revision`. At the final page:

- equal revisions delete dirty state and repair list topology;
- a newer revision retains the node, snapshots it, and restarts from the current occupied-page head.

Publication commits scalar + revision + dirty obligation atomically. Dirty state means reconsideration is pending, not subscriber delivery. Fanout completion means every addressable page for the snapshotted revision was visited and no newer revision remained; it does not mean condition evaluation or task execution. Freshness is evaluated only at the later attempt. Delivery estimates MUST separate publication, fanout, placement, and execution and may assume finite completion only under quiescent revisions, available budget, and eventual queue capacity.

### 7.6 Manual Source

`manual_trigger` requires configured Manual source, Active unpaused nonexpired state, and allowed mutability; otherwise it returns `ManualSourceDisabled`, `AaaPaused`, or the lifecycle error. It sets the shared latch and requests canonical placement, preserving cadence/cooldown/window and all admission gates. It creates no Manual-specific state.

### 7.7 Schedule Window

```rust
struct ScheduleWindow<BlockNumber> {
    start: BlockNumber,
    end: BlockNumber,
}
```

Validation requires checked `end > start`, checked `end + 1`, inclusive span `end - start + 1 >= MinWindowLength`, `end >= current_block` for newly installed Active state, `start - current_block <= MaxExecutionDelayBlocks` under saturating subtraction, and earliest schedule eligibility `<= end`.

Before `start` the actor is not ready; `start..=end` is eligible; after `end` it closes under Section 2.4. `MaxExecutionDelayBlocks` equals ten years in blocks and bounds both window start and cadence delay.

---

## 8. Scheduler

AAA uses one deterministic event-driven scheduler: one paged FIFO for every actor class, one exact temporal wakeup layer, one block cutoff, and complete-operation admission. It never polls the global actor set. System authority and User economics do not create scheduler priority; both wait in the same ticket order.

### 8.1 Architecture

1. **Tickets**: insertion allocates checked monotonic `NextQueueTicket`; exhaustion returns `QueueTicketExhausted`. After due wakeups and fanout, the pass snapshots `cutoff = NextQueueTicket`; only tickets `< cutoff` may execute.
2. **Single paged FIFO**: `QueueHead`, `QueueTail`, and `QueuePages` own one physical order. Pages carry `QueueEntry { ticket, aaa_id }`; unconsumed physical occupancy, including tombstones, equals `QueueOccupancy <= MaxQueueLength`. Actor class does not select another queue or alter order. Page/index overflow returns `SchedulerIndexExhausted`.
3. **Membership**: `ActorHot.queue_ticket` is the sole live queue membership. A physical entry is live only when its ticket equals that field; otherwise it is a tombstone. Enqueue coalesces while live membership exists.
4. **Placement**: target `<= current_block` uses the FIFO. Target `current_block + 1` may use a late ticket only after cutoff; before cutoff it uses exact wakeup. Later targets use wakeup. Queue saturation preserves readiness through exact next-block wakeup. Queue consumption, attempt mutation/events, and every reachable requeue, retry, cadence, window, or fallback placement commit atomically; placement exhaustion or corruption rolls the attempt back without nonce, funding-snapshot, fee, task, or event effects. Due-wakeup removal and FIFO materialization obey the same atomic rule.
5. **Wakeups**: one actor-keyed pointer addresses bounded pages/buckets and a paged min-heap over distinct blocks. Ordinary wakeup and live ticket are exclusive; one terminal-only wakeup may coexist with a ticket. Insert/pop/remove are bounded by `ceil(log2(MaxActiveActors))` sift steps.
6. **Strict head-of-line**: the scheduler consumes tombstones in order, then either admits the live head or stops actor service for the block. It MUST NOT scan behind a live head for a cheaper actor, replace its ticket, or move it behind later work.
7. **Admitted-head fit**: every Active program is admitted only when its worst-case actor unit fits the global actor-service envelope. Therefore a live head may fail to fit the remaining budget of the current block but cannot be permanently too heavy for a conforming full actor-service envelope. Strict FIFO intentionally prioritizes deterministic age order over minimum per-actor latency and provides no bounded lookahead.
8. **One owner**: lifecycle, cutoff, wakeups, Continuation, scan limits, FIFO order, and at-most-once semantics have one state machine. There is no alternate System queue, retry queue, event inbox, or priority lane.

FIFO discovery returns only `Empty`, `Head`, or `Blocked`; exhausted scan or Weight can never prove `Empty`. The actor pass returns `NoLiveWork`, `Progress`, `BlockedByWeight`, or `BlockedOther`.

For every actor `A` and block `B`:

`executions(A, B) <= 1`

Readiness created after execution receives a ticket at or beyond cutoff. Cyclic actor graphs and Continuation retry use no alternate path.

### 8.2 Execution Flow

Every unit uses a two-dimensional `WeightMeter`. New runs begin at step `0`; suspended runs load Continuation only after `run_state` and admit the suffix. Lifecycle, breaker, timing, terminal, User fee capacity, Weight, attempt, and scan gates apply before execution.

Tombstones drain lazily under scan bounds. `MaxExecutionsPerBlock` counts every admitted attempt regardless of result; it neither reserves Weight nor counts stale/invalid entries. All AAA attempts occur in `on_idle`; they do not reorder ordinary extrinsics and provide no frontrunning, backrunning, sandwich, or other ordering-extraction protection.

### 8.3 Liveness Rules

- Deferred and late entries retain FIFO order and are revalidated at pop.
- A Weight-blocked live head retains its ticket and leads the next block.
- Due wakeup materializes FIFO readiness; exact pointer clears only on matching drain/removal.
- Queue saturation, cooldown, cadence, and pre-window delay preserve one exact future path.
- Pause removes ordinary queue/retry wakeup membership while preserving latch, Continuation, and earlier terminal readiness; resume reconstructs admission.
- Dormant/closed/missing entries are tombstones. Expired entries close before normal work.
- Breaker preserves bounded housekeeping and explicit close/sweep but defers attempts and scheduler-owned cleanup.
- Suspension preserves Continuation and owns the retry path. Signals received during suspension remain for the next run; accepted funding accumulates for the next run.

### 8.4 Budget and Fairness

1. Queue/wakeup uniqueness follows actor-local membership; physical tombstones count toward bounded occupancy. Externally controlled churn MUST be fee-accounted where the origin is fee-paying and MUST always be actor-locally rate-limited; System authority creates no exemption in the shared FIFO.
2. Head cleanup skips stale entries and reclaims consumed pages under `MaxQueueEntriesScannedPerBlock`. Page boundaries preserve FIFO.
3. Temporal targets retain exact blocks; live wakeups and distinct blocks remain bounded by `MaxActiveActors`.
4. Runtime guarantees `GuaranteedOnIdleWeight`; hook base is reserved before storage access. Every page, entry, probe, fanout, wakeup, attempt, event, and cleanup is charged before mutation.
5. `WakeupWeightLimit`/`MaxWakeupsPerBlock` and `ObservationFanoutWeightLimit`/`MaxObservationFanoutPagesPerBlock` are hard worker ceilings. Wakeup work is additionally bounded component-wise by the actual `on_idle` remainder after fixed base and saturated queue cleanup; no wakeup mutation may occur unless its complete unit fits both bounds, and returned hook Weight never exceeds the caller remainder. Runtime configuration MUST prove that fixed hook base plus both maximum worker envelopes plus one maximum actor probe-and-attempt-or-cleanup unit fit `GuaranteedOnIdleWeight` component-wise.
6. Workers stop at their own ceilings even when unused actor headroom exists. Actor service receives the remaining budget; there is no guarantee lending or class-specific reserve machinery.
7. Implementations distinguish scans, probes, attempts, deferrals, tombstones, and page operations. `MaxQueueEntriesScannedPerBlock` is independent from `MaxExecutionsPerBlock`.
8. Failure to prove the current head because scan or Weight is exhausted returns `Blocked`, never `Empty`.
9. Under recurring conforming budget, finite tombstone churn, and no external lifecycle blockage, every continuously ready actor eventually reaches the strict FIFO head and receives an admission attempt.

### 8.5 Sweep

`permissionless_sweep` is O(1); `permissionless_sweep_many` is O(K <= MaxSweepBatch). They evaluate terminal lifecycle only, never enqueue or execute a normal cycle, and remain effective under the breaker. Missing/already closed ids count as missing without aborting a batch.

### 8.6 Starvation Safeguard

1. Wakeup processing uses the exact minimum cursor, the component-wise minimum of `WakeupWeightLimit` and the actual post-base/post-cleanup hook remainder, and `MaxWakeupsPerBlock`; it does not scan sparse blocks and cannot consume the statically proven maximum actor unit.
2. Closing uses the actor pointer to invalidate exact wakeup state and repair empty page/bucket/heap topology.
3. After fixed-base and worker processing, `Healthy -> Starving { since }` occurs only when breaker is inactive, live FIFO work exists, no attempt was admitted, and the actor pass returns `BlockedByWeight`.
4. Duration derives from `since`; unchanged starving blocks do not rewrite. Threshold crossing emits one `IdleStarvationDetected` and enters `Alerted`.
5. Breaker activation, no live work, any admitted attempt, or a non-Weight result clears state once; recovery from Alerted emits one `IdleStarvationRecovered`.
6. Telemetry never changes execution authority or creates an alternate scheduler path.

## 9. Runtime Hooks

### 9.1 `on_initialize`

`on_initialize` MUST remain bounded and deterministic, MUST NOT dispatch AAA cycles, and MAY perform minimal bookkeeping only.

### 9.2 `on_idle`

Canonical order:

1. reserve generated fixed hook base before storage access;
2. process bounded due wakeups and lazy physical cleanup within the wakeup worker ceiling;
3. process bounded observation fanout within the fanout worker ceiling;
4. snapshot queue-ticket cutoff;
5. when breaker is inactive, drain the single strict FIFO and admit scheduler-owned terminal cleanup; otherwise skip actor work;
6. update `IdleStarvationState` from the actor-service result.

The static runtime relation in Section 8.4 preserves one maximum actor unit after both workers consume their full envelopes. There are no System/User service phases, class guarantees, or lending rules. Every cursor, page, entry, actor probe, attempt, and cleanup unit MUST fit both Weight dimensions before mutation. Direct producer ingress has no deferred hook drain. No phase may contain an unbounded or unmetered loop.

## 10. Extrinsics

### 10.1 Owner / Control

- `create_user_aaa(mutability, program)`: allocate a fresh `aaa_id` and create at the lowest free owner slot.
- `create_user_aaa_at_slot(owner_slot, mutability, program)`: allocate a fresh `aaa_id` and create at the exact free custody-recovery slot.
- `activate_aaa(aaa_id, active_program)`: install complete Active state on a Mutable Dormant identity.
- `deactivate_aaa(aaa_id)`: remove Mutable Active state while preserving identity, sovereign locator, nonce, and balances.
- `pause_aaa(aaa_id)` / `resume_aaa(aaa_id)`: change Mutable Active lifecycle.
- `manual_trigger(aaa_id)`: set the shared latch when Manual is configured.
- `close_aaa(aaa_id)`: control-origin destruction in place for a Mutable actor.
- `update_schedule(aaa_id, schedule, schedule_window)`: replace on semantic change, reset `schedule_anchor`, cancel Continuation, emit once; exact no-op changes nothing.
- `update_execution_plan(aaa_id, execution_plan, completion_policy)`: replace on semantic change, cancel Continuation, reset `consecutive_failures` because failure history belongs to the replaced program image, recompute tracked funding assets and admission caches, emit once; exact no-op changes nothing.
- `update_funding_source_policy(aaa_id, policy)`: replace on semantic change, cancel Continuation, preserve current accumulated values; exact no-op changes nothing.
- `set_auto_close_at_cycle_nonce(aaa_id, target)`: set/clear target; `Some(target)` requires checked `1 <= target - cycle_nonce <= MaxAutoCloseNonceHorizon`.
- `increment_auto_close_nonce(aaa_id, by)`: checked extension from stored target or current nonce; `by > 0`, resulting delta within horizon.
- `cancel_continuation(aaa_id)`: cancel one Mutable suspended run.

Every call that may close inline declares Weight covering pure cleanup. Both User creation calls pay normal transaction fees and `AaaCreationFee` and enforce all program/horizon/capacity checks before mutation.

Active creation/activation fails with `ActiveAaaCapacityExceeded` at `ActiveActorLimit`. Every creation path fails with `ActorIdentityCapacityExceeded` at `MaxActorIdentities`. User exact-slot creation requires a free slot. Capacity checks use O(1) counters.

### 10.2 Governance

- `create_system_aaa(owner, mutability, program)`: allocate a fresh actor id and fresh System sovereign locator, with `system_sovereign_id = aaa_id`; create Mutable Active/Dormant or Immutable Active actor.
- `create_system_aaa_at_sovereign_id(system_sovereign_id, owner, mutability, program)`: require an allocated vacant locator, allocate a fresh `aaa_id`, and create a new actor over the same sovereign account.
- `set_global_circuit_breaker(paused)`.
- `set_active_actor_limit(new_limit)`: require `ActiveAaaCount <= new_limit <= min(MaxActiveActors, MaxQueueLength)` and `new_limit > 0`.

Fresh System creation increments `SystemSovereignCount` and fails at `MaxSystemSovereigns`; locator reuse does not change that count. Neither System path accepts an explicit actor id. Active-limit validation maps zero, hard-bound overflow, queue-bound overflow, and below-current values to their specific errors and never force-closes actors.

### 10.3 Tooling

- `permissionless_sweep(aaa_id)`.
- `permissionless_sweep_many(ids)` with `len <= MaxSweepBatch`.

System sovereign derivation is a pure public helper over `system_sovereign_id`; tooling and clients do not need an asset-recovery extrinsic to calculate the custody address.

### 10.4 Circuit Breaker

When active:

1. normal attempts and scheduler-owned terminal cleanup stop;
2. bounded queue/wakeup/fanout housekeeping MAY continue;
3. User/System creation, System locator reuse, and Dormant activation fail with `GlobalCircuitBreakerActive`;
4. inbound transfers, Manual signals, explicit Mutable close, and sweep remain available;
5. queued work resumes only after clearing the breaker.

## 11. Observability

Runtime observability reports committed transitions, amounts, assets, retry classes, cursors, and results. It MUST NOT assert profitability, causal significance, market fairness, goal attainment, feedback stability, or other interpretation.

### 11.1 Events

```rust
AaaCreated { aaa_id, owner, actor_class, mutability, sovereign_account, initial_lifecycle: InitialLifecycle }
AaaActivated { aaa_id }
AaaDeactivated { aaa_id }
AaaPaused { aaa_id, reason: PauseReason }
AaaResumed { aaa_id }
AaaClosed { aaa_id, reason: CloseReason }
CycleDeferred { aaa_id, candidate_cycle_nonce, candidate_attempt, cursor, reason: DeferReason }
CycleStarted { aaa_id, cycle_nonce }
CycleSummary { aaa_id, cycle_nonce, result: CycleResult, executed_steps, committed_effectful_tasks, skipped_conditions, skipped_resolution, skipped_funding_unavailable, failed_steps }
StepSkipped { aaa_id, cycle_nonce, step_index, reason: StepSkippedReason }
StepFailed { aaa_id, cycle_nonce, step_index, retry_class: RetryClass, error: DispatchError }
TransferExecuted { aaa_id, cycle_nonce, step_index, asset, amount, to }
SplitTransferExecuted { aaa_id, cycle_nonce, step_index, asset, total, distributed, retained, legs: u32, effective_legs: u32 }
SwapExecuted { aaa_id, cycle_nonce, step_index, asset_in, asset_out, amount_in, amount_out }
BurnExecuted { aaa_id, cycle_nonce, step_index, asset, amount }
MintExecuted { aaa_id, cycle_nonce, step_index, asset, amount }
StakeExecuted { aaa_id, cycle_nonce, step_index, asset, amount }
UnstakeExecuted { aaa_id, cycle_nonce, step_index, asset, shares }
LiquidityDonated { aaa_id, cycle_nonce, step_index, asset_a, asset_b, max_amount_a, max_amount_b, amount_a, amount_b }
LiquidityAdded { aaa_id, cycle_nonce, step_index, asset_a, asset_b, amount_a, amount_b, lp_minted }
LiquidityRemoved { aaa_id, cycle_nonce, step_index, lp_asset, lp_amount, asset_a, asset_b, amount_a, amount_b }
ScheduleUpdated { aaa_id }
ExecutionPlanUpdated { aaa_id, completion_policy }
AutoCloseNonceSet { aaa_id, target: Option<u64> }
AutoCloseNonceIncremented { aaa_id, old_target: Option<u64>, new_target: u64, by: u64 }
ActiveActorLimitSet { old_limit: u32, new_limit: u32 }
GlobalCircuitBreakerSet { paused: bool }
ManualTriggerSet { aaa_id }
SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 }
IdleStarvationDetected { consecutive_blocks: BlockNumber }
IdleStarvationRecovered { consecutive_blocks: BlockNumber }
FundingSourcePolicyUpdated { aaa_id }
FundingAccumulated { aaa_id, asset, added, accumulated }
CycleSuspended { aaa_id, cycle_nonce, attempt, cursor, reason: SuspensionReason, cumulative_outcomes }
CycleContinued { aaa_id, cycle_nonce, attempt, cursor }
CycleCancelled { aaa_id, cycle_nonce, reason: CancellationReason }
CycleStopped { aaa_id, cycle_nonce, step_index }
```

`AaaCreated` covers every fresh actor identity. For System actors, `actor_class` exposes the stable `sovereign_id`; locator reuse therefore remains distinguishable from actor-id continuity. `ManualTriggerSet` emits only on `false -> true`. `FundingAccumulated` emits only for authoritative accepted funding and records the new checked total. Every economic event carries logical-run key and step index; attempt derives from attempt-boundary events.

### 11.2 Correlation and Ordering

Logical run key is `(aaa_id, cycle_nonce)`; attempt key is `(aaa_id, cycle_nonce, attempt)`. `Completed` means terminal completion of authored control flow, including `StopCycle` and completion containing `ContinueNextStep` failures. `Failed` means unsuccessful termination. `Cancelled` means invalidation without completion/failure accounting.

`CycleDeferred.candidate_cycle_nonce` is checked `identity.cycle_nonce + 1` for a new run and current nonce for Continuation. Candidate attempt is `0` or checked next retry attempt; Continuation retains its current cursor. `DeferReason` identifies `RefTime`, `ProofSize`, or `Both`. Emission mutates no run state, requires the complete generated event envelope to fit after actor probes, and consumes that envelope from the actor meter; when it does not fit, neither event nor mutation occurs.

Ordering:

1. Opening: atomically consume the current funding accumulator into the run snapshot, emit `CycleStarted`, then step/effect events, optional `CycleStopped`, and either `CycleSuspended` or terminal finalization.
2. Retry: `CycleContinued`, suffix events, optional `CycleStopped`, then suspension or finalization; no second `CycleStarted` and no second funding-delta consumption.
3. Completed finalization: `CycleSummary(Completed)`, then optional `AaaClosed`.
4. Failed finalization: `CycleSummary(Failed)`, then optional close.
5. Cancellation: `CycleCancelled`, `CycleSummary(Cancelled)`, then optional close.
6. Pure close without an open run: `AaaClosed` only.

Unbounded attempt history is indexed off-chain; current Continuation and current accumulator are canonical on-chain truth.

## 12. Type Reference

### 12.1 Core Types

The following definitions own public variant and field meaning. Before launch, accepted cleanup may change them as one complete metadata epoch; after launch, aliases may specialize bounds only when encoded order remains unchanged.

```rust
type AaaId = u64;
type OwnerSlot = u8;
type OwnerSlotBitmap = [u8; 32];
type SystemSovereignId = u64;
type QueueTicket = u64;
type ObservationRevision = u64;
type BoundedTriggerSources<AccountId, AssetId, ObservationFeedId> =
    BoundedVec<TriggerSource<AccountId, AssetId, ObservationFeedId>, MaxTriggerSources>;

enum AaaType { User, System }
enum ActorClass {
    User { owner_slot: OwnerSlot },
    System { sovereign_id: SystemSovereignId },
}
enum SystemSovereignState { Vacant, Occupied(AaaId) }
enum Mutability { Mutable, Immutable }
enum ActiveLifecycle { Active, Paused(PauseReason) }
enum ActorLifecycle { Dormant, Active, Paused(PauseReason) }
enum InitialLifecycle { Dormant, Active }
enum CompletionPolicy { Persistent, CloseAfterProductiveRun }

struct ActiveProgramInput<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId> {
    schedule: Schedule<AccountId, AssetId, ObservationFeedId>,
    schedule_window: Option<ScheduleWindow<BlockNumber>>,
    execution_plan: BoundedVec<Step<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId>, MaxExecutionPlanSteps>,
    completion_policy: CompletionPolicy,
    funding_source_policy: FundingSourcePolicy<AccountId>,
    auto_close_at_cycle_nonce: Option<u64>,
}

enum ProgramInput<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId> {
    Dormant,
    Active(ActiveProgramInput<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId>),
}

struct ScheduleWindow<BlockNumber> { start: BlockNumber, end: BlockNumber }
struct Schedule<AccountId, AssetId, ObservationFeedId> {
    trigger_policy: TriggerPolicy<BoundedTriggerSources<AccountId, AssetId, ObservationFeedId>>,
    cooldown_blocks: u32,
}

enum TriggerSource<AccountId, AssetId, ObservationFeedId> {
    Manual,
    OnAddressEvent {
        source_filter: SourceFilter<AccountId>,
        asset_filter: AssetFilter<AssetId>,
    },
    OnObservationChange { feed: ObservationFeedId },
}

enum CadenceMode<Sources> { Always, WhenSignalled(Sources) }
enum TriggerPolicy<Sources> {
    Immediate { sources: Sources },
    Cadenced { every_blocks: u32, mode: CadenceMode<Sources> },
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

enum FundingSourcePolicy<AccountId> {
    OwnerOnly,
    SignedAllowlist(BoundedBTreeSet<AccountId, MaxWhitelistSize>),
    RuntimePolicy,
    AnyVerifiedIngress,
}

struct AddressEvent<AccountId, AssetId, Balance, Provenance> {
    destination: AccountId,
    source: Option<AccountId>,
    asset: AssetId,
    amount: Balance,
    provenance: Option<Provenance>,
}

enum AmountResolution<Balance> {
    Fixed(Balance),
    PercentageOfCurrent(Perbill),
    PercentageOfTrigger(Perbill),
    PercentageOfLastFunding(Perbill),
    AllBalance,
}

enum ResolutionSurface<AssetId> {
    Asset(AssetId),
    StakingShares(AssetId),
}

struct SplitLeg<AccountId> { to: AccountId, share: Perbill }

struct Step<AccountId, AssetId, Balance, BlockNumber, ObservationFeedId> {
    conditions: ConditionSet<Condition<AssetId, Balance, BlockNumber, ObservationFeedId>, MaxConditionsPerStep>,
    task: Task<AccountId, AssetId, Balance>,
    on_error: StepErrorPolicy,
}

enum ConditionSet<C, MaxConditions> {
    Always,
    All(BoundedVec<C, MaxConditions>),
    Any(BoundedVec<C, MaxConditions>),
}

enum Condition<AssetId, Balance, BlockNumber, ObservationFeedId> {
    BalanceAbove { asset: AssetId, threshold: Balance },
    BalanceBelow { asset: AssetId, threshold: Balance },
    BalanceEquals { asset: AssetId, threshold: Balance },
    BalanceNotEquals { asset: AssetId, threshold: Balance },
    BlockNumberAbove { threshold: BlockNumber },
    BlockNumberBelow { threshold: BlockNumber },
    ObservationAbove { feed: ObservationFeedId, threshold: u128, max_age_blocks: u32 },
    ObservationBelow { feed: ObservationFeedId, threshold: u128, max_age_blocks: u32 },
    ObservationEquals { feed: ObservationFeedId, threshold: u128, max_age_blocks: u32 },
    ObservationNotEquals { feed: ObservationFeedId, threshold: u128, max_age_blocks: u32 },
}

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

enum StepErrorPolicy {
    AbortCycle,
    ContinueNextStep,
    RetryLater { max_attempts: u32 },
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
    AutoCloseNonceReached,
    BalanceExhausted,
    ConsecutiveFailures,
    CycleNonceExhausted,
    FeeBudgetExhausted,
    OwnerInitiated,
    WindowExpired,
    RetryAttemptsExhausted,
    ProductiveRunCompleted,
}

enum CycleResult { Completed, Failed, Cancelled }
enum DeferReason { RefTime, ProofSize, Both }
enum RetryClass { Permanent, Temporary }
struct TaskFailure { error: DispatchError, retry: RetryClass }
enum SuspensionReason { FundingUnavailable, Temporary }
enum CancellationReason {
    Explicit,
    ExecutionPlanChanged,
    CompletionPolicyChanged,
    FundingPolicyChanged,
    ScheduleChanged,
    Deactivated,
    Closing(CloseReason),
    RuntimeUpgrade,
}
enum StepSkippedReason { ConditionsNotMet, FundingUnavailable, ResolutionSkipped }
enum PauseReason { Manual }

enum Observation<BlockNumber> {
    Fresh { value: u128, observed_at: BlockNumber },
    Stale,
    Uninitialized,
    Unavailable,
}
```

Canonical set rules apply to trigger sources, filters, and funding allowlists. `AaaType::User` holds iff `ActorClass::User { .. }`, and `AaaType::System` holds iff `ActorClass::System { .. }`; `AaaType` is never stored as a competing authority. `ActiveLifecycle` is the stored Active-epoch field, while `ActorLifecycle` is a read-model projection derived from identity presence plus optional `ActorHot`. `InitialLifecycle` excludes Paused by construction. No individual numeric index is normative before the launch compatibility epoch; exact metadata is the complete authority.

### 12.2 Errors

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
    QueueMutationRateLimited,
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
    InvalidTriggerAmountCompatibility,
    AdmissionBoundOverflow,
}
```

Resolution outcomes and `TaskFailure` are not pallet `Error` variants. Exact error selection follows the owning section. The complete numeric manifest is metadata-generated and frozen only at the launch compatibility epoch.

## 13. Storage

All collections are bounded by explicit `Max*` constants. Unless stated otherwise, `Map<K, V>` uses `Blake2_128Concat(SCALE(K))`; changing hasher or key encoding changes storage layout. `BoundedBTreeSet` and `BoundedBTreeMap` MUST encode as canonical ascending-key sequences; insertion-order or unordered encodings are non-conforming.

`0.7.10` defines a fresh pre-launch baseline and therefore includes no `OnRuntimeUpgrade`, migration cursor, legacy storage reader, queue merge bridge, or compatibility state. Genesis, fixtures, metadata, and generated artifacts MUST target only the canonical topology below. After launch, a future storage change follows the versioned migration contract in Section 1.12.

### 13.1 Actor, Custody, and Lifecycle

- `StorageVersion`.
- `NextAaaId: AaaId`: checked monotonic fresh-id allocator; actor ids are never reused.
- `ActorIdentity: Map<AaaId, Identity>`: class and sovereign locator, mutability, owner, sovereign account, durable `cycle_nonce`; present for Active and Dormant identities.
- `ActorHot: Map<AaaId, HotState>`: Active/Paused lifecycle, scheduling clocks, rate-limit clock, queue/wakeup pointers, terminal target, failures/lease, `run_state`, admission bounds, and funding tracked-count cache; absent for Dormant.
- `ActorProgram: Map<AaaId, Program>`: schedule/window, completion policy, bounded plan; absent for Dormant.
- `ActorFunding: Map<AaaId, FundingState>`: policy, tracked assets, and authoritative accumulated funding since the previous run opening; absent for Dormant.
- `ContinuationState: Map<AaaId, ContinuationState>`: unresolved cursor, attempts, suffix trigger/funding snapshots, and cumulative outcomes; present iff `ActorHot.run_state == Suspended`.
- `ActorIdentityCount: u32`: exact Active + Dormant identity count, bounded by `MaxActorIdentities`.
- `ActiveAaaCount: u32`: exact Active + Paused program count.
- `OwnerSlotBitmaps: Map<AccountId, OwnerSlotBitmap>`: fixed 256-bit User custody locator occupancy; bits outside `0..MaxOwnerSlots` are zero and all-zero values are deleted.
- `SystemSovereigns: Map<SystemSovereignId, SystemSovereignState>`: persistent `Vacant | Occupied(AaaId)` registry.
- `SystemSovereignCount: u32`: exact lifetime number of allocated System locator entries, bounded by `MaxSystemSovereigns`; close does not decrement it and `Vacant` entries remain capacity-consuming.
- `SovereignIndex: Map<AccountId, AaaId>`: current Active/Dormant sovereign ownership only; absent for closed vacant locators.
- `ActiveActorLimit: u32`: explicit nonzero operational cap.

User close releases a slot. System close marks its registry entry `Vacant`. Reattachment creates a fresh actor id and updates only current ownership; no closed actor state or nonce is retained.

### 13.2 Queue and Wakeups

- `NextQueueTicket: QueueTicket`: checked monotonic FIFO age allocator and cutoff source.
- `QueueOccupancy: u32`: exact unconsumed physical occupancy, including tombstones.
- `QueueHead/QueueTail: u64`: checked physical positions for the single FIFO.
- `QueuePages: Map<QueuePageId, QueuePage>`: `QueuePageSize`-bounded entries plus consumption metadata.
- `WakeupPages: Map<(BlockNumber, WakeupPageId), WakeupPage>`: fixed optional slots, live count, drain cursor, reciprocal page links.
- `WakeupBuckets: Map<BlockNumber, WakeupBucketState>`: exact page/live metadata for one target block.
- `WakeupCursorPages` and `WakeupCursorLen`: paged min-heap over distinct target blocks.
- `ActorHot.queue_ticket`: sole live FIFO membership.
- `ActorHot.wakeup_pointer`: sole temporal membership; one terminal-only wakeup may coexist with a ticket.
- `ActorHot.pending_signal`: sole source/fanout readiness latch.

Queue pages are bounded by `MaxQueueLength`; live wakeups and distinct target blocks by `MaxActiveActors`. Any operation requiring a new ticket/page/index MUST preflight namespace availability before invalidating current membership or committing producer movement.

### 13.3 Observation Delivery

- `ActorObservationFeeds: Map<AaaId, BoundedVec<ObservationFeedId, MaxTriggerSources>>`.
- `ActorObservationSlot: Map<AaaId, ObservationSlot>` and `ObservationSlotOwner: Map<ObservationSlot, AaaId>`.
- `ObservationNextSlot`, `ObservationFreeSlotPages`, `ObservationFreeSlotTopPage`: paged reusable-slot allocator.
- `ObservationSubscriberPages: Map<(ObservationFeedId, SubscriberPageId), SubscriberPage>`: fixed cells and reciprocal occupied-page links.
- `ObservationFeedPages: Map<ObservationFeedId, FeedPageTopology>`: occupied-page topology and subscriber count.
- `ObservationSubscriptionCount: u32`: exact global count, bounded by `MaxActiveActors * MaxTriggerSources`.
- `ObservationIngressRevisions: Map<ObservationFeedId, ObservationRevision>`: highest accepted revision for continuously subscribed feeds.
- `DirtyObservations: Map<ObservationFeedId, DirtyObservationState>`: latest/fanout revisions, dirty block, next page, active-list links.
- `DirtyObservationHead`, `DirtyObservationTail`, `DirtyObservationCursor`, `DirtyObservationCount`.

Empty pages, zero feed topology, released slots, and completed dirty states are deleted immediately. A clean subscribed feed retains only its revision baseline; an unsubscribed feed retains neither baseline nor dirty state. Try-state reconciles every actor/feed/slot/page/revision/count/link relation.

### 13.4 Global State and Compatibility

- `GlobalCircuitBreaker: bool`.
- `IdleStarvationState: Healthy | Starving { since } | Alerted { since }`.

`ActorIdentity.cycle_nonce`, `ActorClass` sovereign locators, `SystemSovereigns`, `ActorHot.run_state`, completion policy, Continuation key/value, funding accumulator meaning, single-FIFO order, observation revision baselines/topology, and dirty progress are compatibility-significant after launch. The exact metadata-generated ABI manifest MUST be checked in CI; prose defines no partial numeric registry.

## 14. Conformance

Compliance requires executable evidence against production metadata and Wasm.

### 14.1 Release Gates

| Gate | Evidence |
| --- | --- |
| `G-METADATA` | Complete call/event/error/type manifest; pre-launch drift is accepted only as one reviewed epoch, launch freezes the complete manifest, and CI detects later discriminant, field-order, and bound drift. |
| `G-WASM` | RefTime and ProofSize benchmarks cover hook bases, single-FIFO pages, wakeups, probes, every step outcome, adapter branch, ingress, rollback, snapshots, Continuation, finalization, and cleanup. |
| `G-EMBEDDING` | Typed adapters and host services pass lifecycle, funding-delta, Continuation, single-FIFO scheduler, and observation tests in an independent runtime embedding. |
| `G-EXECUTIVE` | Executive tests prove paired ingress rollback, post-dispatch Weight correction without AAA-fee refund, identical transfers, source-less provenance, and every crediting adapter path; evidence includes the complete crediting-producer inventory required by Section 3.5. |
| `G-MODEL` | Property-based state-machine exploration covers lifecycle, retry/cancellation, locator reuse, the 256-bit owner-slot namespace, configurable shared plan bounds, tombstones, wakeup replacement/coexistence, breaker, observation churn, namespace edges, and close precedence. Required cases include lowest-free allocation across bitmap byte boundaries, exact slot `254`, rejection of slot `255`, configured plan-bound and hard-ceiling edges, partial fanout page + actor deactivation, subscriber mutation during fanout, newer revision during page progress, queue saturation, ticket + terminal wakeup, stale entries after close/recreation, plan or schedule replacement during Continuation, User fee-native debit at the protected floor, fee-collector failure after admission, invalid `Fresh`, nonce exhaustion for both classes, and a heavy FIFO head deferred until the next conforming envelope without follower reordering. |
| `G-FRESH-BASELINE` | The release constructs only the canonical identity, custody, funding-delta, single-FIFO, observation, and bitmap topology from genesis/fixtures; no migration entrypoint, migration state, legacy decoder, dual write, or compatibility shadow exists. |

After launch, future releases add upgrade and migration evidence as required by Sections 1.12, 2.5, and 13. They are not `0.7.10` release gates because no launched AAA state predates this baseline.

### 14.2 Required Invariants

| Id | Required property |
| --- | --- |
| `ID-UNIQUE` | Fresh actor ids and queue tickets are checked monotonic and never reused; `(aaa_id, cycle_nonce)` never repeats even when custody locators are reused. |
| `ID-SLOT` | User slots use one canonical 256-bit bitmap, admit only `owner_slot < MaxOwnerSlots <= 255`, select the lowest free bit through a bounded 32-byte scan, clear synchronously, and reconcile with `ActorClass`; System consumes none. |
| `ID-SOVEREIGN` | User/System derivation is deterministic and total; live actor collision, vacant/occupied System locator state, and reserved-account rejection are distinct. |
| `ID-REATTACH` | User exact-slot and System sovereign-id reuse allocate a fresh actor id, preserve the same sovereign account, inherit no actor state or guarantees, and transfer authority over residual adapter-exposed custody state to the new actor. |
| `ID-COUNT` | Identity, Active, System sovereign, queue, subscription, and dirty counts equal detailed topology and remain bounded. |
| `LIFE-DORMANT` | Dormant identity owns no program, funding accumulator, readiness, Continuation, subscription, recurring fee, or cycle state. |
| `LIFE-DEACTIVATE` | Deactivation preserves identity, locator occupancy, nonce, and balances while removing the complete Active epoch. |
| `LIFE-CLOSE` | Every close prevalidates then performs one pure deletion, releases the locator for explicit reattachment, preserves balances, and emits once. |
| `LIFE-WINDOW` | Inclusive windows have direct `end + 1` readiness that survives FIFO backlog. |
| `LIFE-PRECEDENCE` | Window, balance/fee, nonce, exact retry/global threshold algorithm, productive completion, and lease close order matches Section 2.4. |
| `LIFE-NONCE` | `u64::MAX` is terminal for either class before another Active installation or run; no nonce-exhausted paused state exists. |
| `LIFE-BREAKER` | Breaker halts attempts/scheduler cleanup without blocking bounded housekeeping, ingress, explicit Mutable close, or sweep. |
| `LIFE-IMMUTABLE` | Immutable actors expose no control mutation under the current dispatch contract and may remain Active indefinitely when no internal terminal condition is reachable. |
| `EXEC-PLAN-BOUND` | One runtime `MaxExecutionPlanSteps` in `1..=255` applies to both classes; the baseline is `8`; any binding change regenerates metadata/weights and revalidates Active envelopes before execution. |
| `EXEC-CONTROL` | Cursor never decreases; only `StopCycle` completion selects early completed terminal; no other surface selects arbitrary control. |
| `EXEC-COMPLETION` | Reaching terminal through accepted `ContinueNextStep` failures yields `Completed`; counters preserve the factual failures. |
| `EXEC-CONDITION` | One canonical non-nested aggregate owns each step; all atoms are evaluated and cannot mutate state; invalid `Fresh` is Permanent failure rather than false. |
| `EXEC-AMOUNT` | Resolution is exhaustive, floor-rounded, checked, unclamped, rechecks current capacities on retry, and applies the User fee-native protected floor to every direct and auxiliary debit cap. |
| `EXEC-TRIGGER-SNAPSHOT` | Trigger snapshots are opening-time typed balances, suffix-trimmed on suspension, and never event payloads. |
| `EXEC-FUNDING` | Accepted funding applies exactly once to the accumulator; opening consumes the delta only after all fallible admission checks; retries use the frozen suffix snapshot; later funding belongs to the next run; outcome never promotes or restores it. |
| `EXEC-ATOMIC` | Task failure rolls back task-local adapters, movements, ingress, and success events while preserving prior steps and collected attempt fee. |
| `EXEC-CONTINUATION` | Cursor proves committed prefix; `run_state == Suspended` iff one sparse Continuation exists. |
| `EXEC-RETRY` | Only Temporary or `FundingUnavailable` under valid Mutable `RetryLater` suspends; protocol backoff and local/global threshold precedence are exact. |
| `EXEC-CANCEL` | Cancellation emits one cancelled boundary and performs no completion accounting, compensation, funding restoration, prefix rollback, balance movement, or shared scan. |
| `EXEC-FINALIZE` | Attempt admission preflights all fallible finalization/placement/snapshot consequences; finalization cannot fail after the first committed task effect. |
| `EXEC-SPLIT` | Split validation, conservation, all-zero skip, preflight, and whole-task rollback hold. |
| `ECON-FEE` | Both User creation calls charge once; one pure fee-envelope derivation owns admission, reservation, execution, simulation, and conformance vectors; User attempts collect at most once per step, preserve `MinUserBalance`, release the suffix envelope to exactly zero, retain attempt fees after task rollback, and keep post-dispatch Weight correction separate. Fee-collector failure commits no debit, emits Permanent step failure, and follows policy without dispatch or recollection. |
| `ECON-NO-RENT` | No recurring rent, automatic refund, recovery transfer, or post-close balance movement exists. |
| `EVENT-ORDER` | One nonce spans attempts; effect events carry nonce/index; opening snapshot, summary, cancellation, and close ordering is exact. |
| `SCHED-METER` | No hook unit mutates before complete RefTime/ProofSize admission. |
| `SCHED-ONE-FIFO` | User and System actors share one paged FIFO; actor class never changes ticket order or service rights. |
| `SCHED-MEMBERSHIP` | One actor ticket and one temporal pointer are authoritative; only ticket + terminal-only wakeup may coexist. |
| `SCHED-CUTOFF` | Post-worker cutoff enforces `executions(A, B) <= 1`; late readiness persists. |
| `SCHED-TIME` | Immediate, cadence, cooldown, fixed retry backoff, window, capacity retry, and expiry use one exact placement contract. |
| `SCHED-HOL` | Strict FIFO head-of-line blocking holds; a live head is never bypassed and `Blocked` never becomes `Empty`. Structural step bounds reduce maximum unit size, while production Weight admission proves that a conforming head is never permanently oversized. |
| `SCHED-WORKERS` | Maximum wakeup and fanout envelopes plus fixed base leave one maximum actor unit inside `GuaranteedOnIdleWeight`; no lending or class-reserve state exists. |
| `SCHED-COUNTERS` | Attempt and scan ceilings remain distinct from Weight and count their defined physical units. |
| `SCHED-NO-POLL` | Normal operation performs no global actor, sparse-block, subscription-slot, or dirty-feed scan. |
| `SCHED-STARVATION` | Telemetry reacts only to live-work Weight blockage, writes on transitions only, and never changes authority. |
| `HOST-CLOSED` | Unbound capabilities fail closed; retry class never derives from raw error encoding. |
| `HOST-WEIGHT` | Adapter/host work is deterministic bounded and never exceeds generated two-dimensional bounds. |
| `HOST-EFFECT` | Successful positive requests commit declared effects and debit only explicit bounded surfaces. |
| `INGRESS-ATOMIC` | Every crediting producer appears in the certified inventory and uses one paired transaction; failure rolls back movement and no-op/self/fee movement creates no ingress. |
| `OBS-SUBSCRIPTION` | Subscriptions derive only from source atoms and reconcile exact actor/feed/slot/page topology. |
| `OBS-DIRTY` | Changed publication, monotonic baseline, and dirty obligation commit atomically; equal replay is no-op and regression fails. |
| `OBS-FANOUT` | One unit touches at most one occupied page, preserves partial progress, rotates fairly, and advances only after future paths exist. |
| `OBS-SEMANTICS` | Signals and revisions coalesce into latest-state reconsideration; no event/revision cardinality or execution promise exists. |
| `DEX-REFERENCE` | Every System swap enforces directed nonzero reference deviation with fixed checked arithmetic and typed failure. |
| `COMPAT-EPOCH` | Before launch no isolated pin exists without a documented consumer; launch freezes one complete metadata manifest; post-launch ABI is append-only. |
| `COMPAT-BASELINE` | `0.7.10` exposes only the fresh canonical baseline and contains no migration or compatibility path for pre-launch repository state. |
| `COMPAT-STORAGE` | After launch, storage changes use an incremented version and bounded idempotent migration. |
| `COMPAT-CONTINUATION` | After launch, upgrades preserve Continuation only through an accepted manifest and semantic proof; otherwise boundedly cancel it. |
| `COMPAT-ADAPTER` | Position/share/LP bindings and failure classes are not reinterpreted in place without migration. |

## 15. Runtime Configuration

### 15.1 Required Bindings

- `AaaPalletId = PalletId(*b"aaactor0")`.
- `FeeNativeAsset`, `FeeCollector`, `FeeSink`, `WeightToFee`.
- `AssetOps`, `DexOps`, `StakingOps`, `LiquidityOps`.
- `ObservationProvider`, changed-revision hook, `FundingAuthority`, `SovereignAccountPolicy`.
- `GuaranteedOnIdleWeight` after maximal admitted extrinsics.
- `WakeupWeightLimit`, `ObservationFanoutWeightLimit`.
- `SystemSwapEmaMaxAgeBlocks`, `SystemSwapMaxReferenceDeviation`.
- target block time.

Retry backoff `1, 2, 4, 8, 8...` is protocol-fixed and has no runtime binding.

### 15.2 Bounds and Relations

Required bounds:

- actors/custody: `MaxActiveActors`, `MaxActorIdentities`, `MaxSystemSovereigns`, `ActiveActorLimit`, `MaxOwnerSlots`;
- program: `MaxExecutionPlanSteps`, `MaxRetryAttempts`, `MaxConsecutiveFailures`, `MaxConditionsPerStep`, `MaxTriggerSources`, `MaxFundingTrackedAssets`, `MaxContinuationSnapshotEntries`, `MaxWhitelistSize`, `MaxSplitTransferLegs`;
- physical: queue/wakeup/cursor/observation page sizes, queue length, per-block wakeup/fanout/scan/attempt/sweep ceilings, and worker Weight limits;
- adapter: every bounded-search `MaxK`;
- time/economics: execution delay, jitter, window length, auto-close horizon, starvation threshold, opening/step/condition fees, `MinUserBalance`.

Relations:

1. `0 < ActiveActorLimit <= min(MaxActiveActors, MaxQueueLength)` and `ActiveActorLimit >= ActiveAaaCount` after governance update.
2. `MaxActorIdentities >= MaxActiveActors`; `MaxSystemSovereigns > 0`; count types represent their bounds.
3. `0 < MaxOwnerSlots <= 255`; `OwnerSlotBitmap` always contains 256 bits, and every bit at an index `>= MaxOwnerSlots` is zero.
4. `0 < MaxExecutionPlanSteps <= 255`. One binding applies to both classes; no User/System plan-length constants exist. The baseline is `8`.
5. `MaxExecutionPlanSteps * MaxRetryAttempts <= u32::MAX`.
6. Unique trigger surfaces fit `MaxContinuationSnapshotEntries <= 2 * MaxExecutionPlanSteps`; persisted funding snapshot entries fit `MaxFundingTrackedAssets`.
7. `MaxActiveActors * MaxTriggerSources <= u32::MAX`.
8. Every page size, collection bound, per-block ceiling, and worker Weight limit is nonzero.
9. `WakeupWeightLimit` and `ObservationFanoutWeightLimit` each cover one worst-case complete unit and remain within their generated hard envelopes.
10. After fixed hook base, `GuaranteedOnIdleWeight` component-wise covers maximum wakeup worker envelope + maximum fanout worker envelope + one maximum actor probe plus attempt or cleanup unit.
11. Every admitted plan, suffix, producer consequence, and cleanup branch fits that global actor-service envelope. A conforming live FIFO head can be deferred by current remainder but cannot be permanently oversized.
12. `SystemSovereignCount <= MaxSystemSovereigns`; the bound covers lifetime allocated locators, including `Vacant` entries. Each occupied registry value names exactly one current System identity whose `ActorClass` carries that locator; each vacant value has no live `SovereignIndex` owner.
13. `SystemSwapEmaMaxAgeBlocks > 0`; `SystemSwapMaxReferenceDeviation < Perbill::one()`.
14. `MinUserBalance >= AssetOps::minimum_balance(FeeNativeAsset)`; `AaaCreationFee > 0`; `StepBaseFee > 0`.
15. `MaxExecutionDelayBlocks = ceil(10 Julian years / target block time)`; one Julian year is `365.25` days. `BlockNumber` and all conversion/intermediate types MUST represent that bound, checked `end + 1`, and every accepted composed target; any target that would overflow its final type is rejected before mutation.
16. `MinWindowLength` counts inclusive blocks.
17. Every loop/storage bound is present in metadata or generated conformance descriptors.

Changing `MaxExecutionPlanSteps` within the hard ceiling is a semantic and capacity change even when SCALE field order remains stable. It requires final metadata regeneration, production Weight evidence, `G-MODEL` replay, and Active-cache revalidation before execution resumes.

### 15.3 Baseline Profile

- `MaxActiveActors = 10_000`
- `MaxOwnerSlots = 255` (valid slots `0..=254`; fixed 256-bit bitmap)
- `MaxConditionsPerStep = 4`
- `MaxSplitTransferLegs = 8`
- `MaxExecutionPlanSteps = 8`
- `MaxContinuationSnapshotEntries = 16`
- `MaxWhitelistSize = 16`
- `MaxSweepBatch = 5`
- `MaxConsecutiveFailures = 10`
- `MaxRetryAttempts = 10`
- `MaxExecutionsPerBlock = 1_000`
- `WakeupPageSize = 32`
- `MinWindowLength = 100` inclusive blocks
- `MaxAutoCloseNonceHorizon = 10_000`
- `GuaranteedOnIdleWeight` derives from 50% block headroom
- `StepBaseFee = 0.002 Native`
- `ConditionReadFee = 0.0005 Native`

`MaxQueueLength`, `MaxSystemSovereigns`, remaining page sizes, scan/fanout/wakeup ceilings, worker Weight limits, and adapter `MaxK` values are production-Wasm and capacity-model outputs, not decorative defaults.

---

_End of specification._
