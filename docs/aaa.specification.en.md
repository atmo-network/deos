# AAA Specification

- **Component**: `pallet-aaa` (Account Abstraction Actors)
- **Specification line**: `0.7.1`
- **Date**: July 2026
- **Status**: Normative

> The key words **MUST**, **REQUIRED**, **SHALL**, **SHOULD**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this document are to be interpreted as described in RFC 2119.

---

## 0. Specification Maintenance Meta-Layer

This specification MUST stay at or below **1280 lines** (formatting-preserving count), add new normative content only with equal-or-greater removal of obsolete content, state rules as positive executable behavior unless a negative safety-critical constraint is required, keep normative facts single-sourced with references instead of duplication, preserve mandatory blank-line separation above and below numbered headings, and ensure every line carries normative meaning, traceability, or required implementation context.

---

## 1. Stability Contract

1. **Determinism**: Identical network state and block context MUST produce identical AAA behavior across all nodes.
2. **Bounded Work**: Every runtime path (`on_initialize`, `on_idle`, extrinsics) MUST execute in O(1) or O(K) with explicit, finite `Max*` constants; hook work MUST reserve its complete two-dimensional `Weight(RefTime, ProofSize)` before execution and MUST stop before either remaining dimension is exhausted.
3. **Destruction in Place**: On terminal conditions, actor state is removed atomically and balances remain on the sovereign account.
4. **No-Refund Contract**: The protocol MUST NOT perform automatic asset refund fan-out on close; balance recovery is owner-operated.
5. **Creation-Cost Internalization**: `create_user_aaa` MUST charge a non-refundable opening fee through the runtime `FeeCollector` into `FeeSink` to cover long-tail maintenance of abandoned actors.
6. **Stateless Execution Plans**: Steps are independent and read state at execution time; mutable cross-step state is forbidden. Read-only per-run execution context (e.g., `reserved_fee_remaining`, `TriggerSnapshot`) is allowed.
7. **Predictable Failure**: Failures MUST resolve into one of: `Deferred`, `StepSkipped`, `StepFailed`, or `AaaClosed`.
8. **Synchronous Mutations**: Slot-allocation mutations MUST be persisted in the same extrinsic execution to prevent intra-block races.
9. **Saturating Arithmetic**: Intermediate fee/limit math MUST use saturating semantics. User-visible amount resolution MUST NOT silently overflow or underflow and MUST resolve deterministically (`Skipped` outcome or explicit failure).
10. **Execution-Context Correctness**: Rules MUST respect FRAME hook semantics (e.g., no reliance on current block hash during execution).
11. **Deferred-Horizon Cap**: Runtime MUST reject configurations that postpone first eligible execution beyond ten years.
12. **Spec-Impl and Compatibility Lock**: Runtime behavior MUST conform to this document in the same release window, and release CI MUST block shipment unless invariant-mapped tests for Section 14 pass. The `0.7.x` pre-launch line remains fresh-genesis and MAY make an explicitly released breaking cleanup without shipping historical migration ceremony. Once AAA `1.0` is declared, existing task/condition/trigger/amount/close/event/error SCALE discriminants and dispatch call indices become append-only; existing call argument encodings and semantics remain stable; storage prefix/hasher/key/value encoding changes require a bounded migration, incremented pallet `StorageVersion`, and runtime compatibility bump; removals or semantic reinterpretations require an AAA major version; additive adapters/configuration or conservative weight recalibration follow Rust/package semver and runtime metadata/spec-version policy without pretending that package version alone migrates a live chain.

## 2. Actor Model

### 2.1 Instance

- **Terminology**: An **Execution Plan** is the static bounded list of steps configured on the actor. An **Execution Run (Cycle)** is one admitted execution attempt of the current plan, identified by `(aaa_id, cycle_nonce)`. All external observability and indexer correlation MUST be run-centric. Execution plans, trigger filters, and actor-to-actor asset flows are part of the on-chain behavioral surface of AAA, but they operate inside the scheduler, fee, lifecycle, and safety contract of this runtime; within existing task, adapter, and safety limits, protocol workflow changes SHOULD prefer actor-graph reconfiguration over runtime rewrites.
- **Native-asset terminology**: `FeeNativeAsset` denotes the balance surface used for `AaaCreationFee`, per-step User fees, `MinUserBalance`, fee collection, and fee reservation. Staking uses the generic `Stake { asset, amount }` task only; any native staking representation is a runtime-defined `AssetId` interpreted by `StakingOps`, not a separate AAA task.
- **Stable plan shape**: An active `execution_plan` MUST be non-empty. `on_close_execution_plan` MAY be empty to express no close-time side effects; Mutable actors MAY replace it through `update_on_close_execution_plan`. Dormant identities store neither plan. Immutable active actors commit both plans at creation and expose no post-creation mutation path.
- **Production-budget admission**: The embedding runtime MUST provide one gross two-dimensional `GuaranteedOnIdleWeight`. Genesis, create, reopen, and either plan update MUST compose the guaranteed scheduler envelope (`scheduler_admission_overhead`) + run cycle + close cycle/cleanup for the prospective plan pair and reject it with `ExecutionPlanExceedsOnIdleBudget` unless both RefTime and ProofSize fit. The envelope includes fixed hook/probe, bounded baseline zombie-scan, queue, wakeup-cursor, and actor-probe work rather than every optional compatibility-ingress drain, heavyweight wakeup-retry, or sweep-time terminal-close unit. Those durable housekeeping paths consume only complete units that fit and MAY defer actor work across blocks without losing the trigger, retry marker, or terminal condition. Admission happens before opening-fee collection or state mutation; bounded vector shape alone never implies production admissibility.

```rust
enum ActorClass { User { owner_slot: u8 }, System }

enum ActiveLifecycle { Active, Paused(PauseReason) }

struct ActorIdentity<AccountId, BlockNumber> {
    class: ActorClass,
    mutability: Mutability,
    sovereign_account: AccountId,
    owner: AccountId,
    created_at: BlockNumber,
}

struct ActorHot<BlockNumber, Balance> {
    lifecycle: ActiveLifecycle,
    auto_close_at_cycle_nonce: Option<u64>,
    cycle_nonce: u64,
    last_cycle_block: BlockNumber,
    consecutive_failures: u32,
    manual_trigger_pending: bool,
    cycle_weight_upper: Weight,
    cycle_fee_upper: Balance,
}

struct ActorProgram {
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow>,
    execution_plan: BoundedVec<Step, MaxSteps>,
    on_close_execution_plan: BoundedVec<Step, MaxSteps>,
}

struct ActorFunding<AccountId, Balance> {
    funding_source_policy: FundingSourcePolicy<AccountId>,
    funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
    funding_snapshots: BoundedBTreeMap<AssetId, FundingBatch<Balance>, MaxFundingTrackedAssets>,
}

struct FundingBatch<Balance> { amount: Balance, pending_amount: Balance }
```

### 2.2 Types and Mutability

- **User AAA**: Subject to evaluation/execution fees and bounded by `MaxOwnerSlots` via user slot allocation.
- **System AAA**: Governance-created, exempt from User fee model, MAY be Mutable or Immutable, MUST NOT be limited by user slot count, and MUST keep `owner_slot = 0` as a storage/event compatibility sentinel.

Mutability rules:

- **Mutable**: control origin MAY pause/resume/update schedule/update execution plan/update on-close execution plan/update funding-source policy/set or increment auto-close target.
- **Control origin**: signed owner for both actor types; governance origin is additionally valid for System AAA only.
- **User Immutable**: owner mutation calls MUST fail with `ImmutableAaa`; a runtime MAY expose emergency governance override for User actors only.
- **System Immutable**: no runtime extrinsic, including governance/root, may mutate, activate/deactivate, pause/resume, manually trigger, close, or reopen the actor; only runtime upgrade may alter the invariant. It MUST be created active.
- Ordinary inbound transfers remain available regardless of actor mutability; mutability does not grant funding authority. `manual_trigger` MUST remain available for User AAA and System Mutable AAA unless another lifecycle gate rejects it; System Immutable `manual_trigger` MUST fail with `ImmutableAaa`.

### 2.3 Sovereign Derivation and Slot Allocation

1. **User AAA**: `seed = Blake2_256( SCALE(AaaPalletId, owner, owner_slot) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`.
2. **System AAA**: `seed = Blake2_256( SCALE(AaaPalletId, b"system", aaa_id) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`. Slotless: MUST NOT consume bits in `OwnerSlotMask`; stored/emitted `owner_slot` MUST remain `0` as a compatibility sentinel and MUST be interpreted together with `aaa_type`.
3. User slot bit MUST be cleared on User AAA destruction.
4. Recreating a User AAA with the same `(owner, owner_slot)` or reopening a closed System AAA with the same `aaa_id` MUST derive the same `sovereign_account`.
5. Collision check MUST guard active or dormant AAA ownership of the same sovereign account; this case MUST fail with `SovereignAccountCollision`.
6. Pre-existing account state on a derived sovereign account (balances, dust, locks, reserves, third-party transfers) MUST be treated as valid and MUST NOT be considered a collision.

- `OwnerSlotMask: Map<AccountId, u8>`
- `MaxOwnerSlots <= 8` (default `8`)
- Bits above `MaxOwnerSlots` MUST be zero
- `valid_mask(n)` denotes a `u8` mask with the lowest `n` bits set
- `create_user_aaa(...)` picks the lowest free slot; `create_user_aaa_at_slot(owner_slot, ...)` requires the exact slot and fails with `InvalidOwnerSlot` or `OwnerSlotOccupied`

_Integration note: The bitmask is Little-Endian SCALE encoded (`(1 << n) - 1`). Before creating or recreating a User AAA, clients SHOULD precompute the target sovereign account and display current balances/locks/reserves; `create_user_aaa_at_slot` is the stable recovery path when execution control must reattach to the original sovereign account._

```rust
let mut mask: u8 = OwnerSlotMask::get(&owner);
mask &= valid_mask(MaxOwnerSlots);
let owner_slot = match preferred_slot {
    Some(slot) if slot >= MaxOwnerSlots => return Err(Error::InvalidOwnerSlot),
    Some(slot) if (mask & (1 << slot)) != 0 => return Err(Error::OwnerSlotOccupied),
    Some(slot) => slot,
    None => (0..MaxOwnerSlots).find(|i| (mask & (1 << i)) == 0).ok_or(Error::OwnerSlotCapacityExceeded)?,
};
mask |= 1 << owner_slot;
OwnerSlotMask::insert(&owner, mask & valid_mask(MaxOwnerSlots));
```

System AAA id rules:

1. `create_system_aaa(mutability, ...)` MUST allocate a fresh `aaa_id` from `NextAaaId`; governance MUST NOT choose an explicit fresh `aaa_id` through the stable surface.
2. `reopen_system_aaa(aaa_id, mutability, ...)` is the only stable explicit-id System AAA creation path. It MAY reopen only a previously closed Mutable System AAA id, the requested new `mutability` MUST also be Mutable, and either Immutable case MUST fail with `ImmutableAaa`; an id without a System close tombstone MUST fail with `SystemAaaNotClosed`.
3. `reopen_system_aaa` MUST fail with `AaaIdOccupied` if the requested `aaa_id` already has an active or dormant identity.
4. System AAA creation/reopen MUST insert `SovereignIndex[sovereign_account] = aaa_id` atomically with actor identity; active or dormant identity occupancy of that sovereign account is the collision criterion, while runtime-declared custody-only accounts remain outside this generic index.
5. `NextAaaId` MUST remain monotonic. Reopening a previously closed lower id MUST NOT rewind it.

### 2.4 Lifecycle

Terminal conditions:

- `fee_native_balance < MinUserBalance`: before cycle start → User `AaaClosed(BalanceExhausted)`
- `consecutive_failures >= MaxConsecutiveFailures`: after cycle failure → `AaaClosed(ConsecutiveFailures)`
- `current_block > schedule_window.end`: all touch points → `AaaClosed(WindowExpired)`
- `fee_native_balance < cycle_fee_upper`: scheduler admission → User `AaaClosed(FeeBudgetExhausted)`
- stored `cycle_nonce == u64::MAX`: before any further normal-cycle admission → User closes, System pauses with `CycleNonceExhausted`
- auto-close target reached after successful run → `AaaClosed(AutoCloseNonceReached)`

System AAA is exempt from `MinUserBalance` checks and MUST NOT auto-pause on `FundingUnavailable`; unresolved funding is modeled as `StepSkipped(FundingUnavailable)`. Runtime configuration MUST enforce `MinUserBalance >= ExistentialDeposit(FeeNativeAsset)`. System Mutable owner/governance `close_aaa` MUST stay available without funding/trigger preconditions to remove long-paused actors. System Immutable `close_aaa` MUST fail with `ImmutableAaa`, but this control-extrinsic guard MUST NOT block mandatory internal terminal transitions such as `ConsecutiveFailures` closure.

`WindowExpired` MUST be evaluated at every lifecycle touch point (scheduler admission, sweep extrinsics, `manual_trigger`, pause/resume, schedule/execution-plan/funding-policy update). `schedule_window = None` never expires. If `current_block > schedule_window.end`, runtime closes before and instead of other mutations in that call. Ordinary transfers remain balance movements rather than lifecycle touchpoints; ingress for an expired actor MUST NOT arm new funding state before bounded lifecycle closure. Schedule window eligibility is inclusive on `end`: `start <= current_block <= end`; closure starts only when `current_block > end`.

Lifecycle separates durable actor identity from an optional executable program. `ActorClass` is `User { owner_slot } | System`, with mutability stored independently; implementations MUST reject or make unrepresentable a User without a valid occupied slot and any Immutable actor without an active program. Program lifecycle is `Dormant | Active | Paused(PauseReason)`. `Dormant` retains `aaa_id`, class, owner, sovereign account, creation lineage, and owner-slot occupancy but owns no schedule, window, plans, funding policy or batches, cached plan bounds, readiness mirror, queue ticket, wakeup pointer, inbox latch, cycle/failure/lease counters, fee work, or periodic events. A runtime-specific protocol/custody account that needs only deterministic account derivation MUST remain outside actor identity storage entirely.

Active lifecycle is `Created → Active → Ready → Admitted → Running → Completed/Deferred/Failed → TerminalPending → Closing → Closed`; pause is an active-program state rather than dormancy. Normal cycles are scheduler-owned runs of `execution_plan`, increment `cycle_nonce` at admission, and emit `CycleStarted`/`CycleSummary`. Close tails are terminal runs of `on_close_execution_plan`; they are not normal cycles, MUST NOT increment `cycle_nonce`, and emit close-tail events followed by `AaaClosed`. Lifecycle touch extrinsics MAY detect terminal state and enter the close path, but MUST NOT present that path as a normal cycle.

Creation and reopen take one typed `ProgramInput`: `Dormant`, or `Active { schedule, schedule_window, execution_plan, on_close_execution_plan, funding_source_policy }`. `Dormant` MUST be rejected for both Immutable classes. Active input MUST validate the complete class policy, trigger, window, both plans, funding policy, tracked assets, cached weight/fee bounds, deferred horizon, and guaranteed production admission envelope before writing actor, scheduler, readiness, inbox, queue, or wakeup state. Dormant creation performs no plan scan, fee forecast, readiness calculation, or scheduler enrollment.

A Mutable active actor MAY transition to Dormant through `deactivate_aaa`. Deactivation MUST invalidate all canonical and derived scheduler membership, remove the executable program and funding state, clear cycle/failure/lease state, preserve identity, sovereign balances, and User slot occupancy, decrement only active cardinality, and emit exactly one `AaaDeactivated` transition event; it MUST NOT execute either plan or emit cycle/close events. A Mutable dormant actor MAY transition through `activate_aaa` using the complete Active input above; activation MUST finish all validation and active-capacity admission before any mutation, then install the program and scheduler state atomically and emit exactly one `AaaActivated` event. Inbound value to a dormant or custody-only sovereign account is balance-only and MUST NOT create funding, inbox, queue, wakeup, fee, or cycle work. Pause/resume apply only to active programs; System Immutable actors cannot activate, deactivate, pause, resume, close, or reopen through mutable control paths.

`ActorIdentityCount` MUST count active plus dormant actor identities against a hard `MaxActorIdentities` bound. `ActiveAaaCount` MUST count active and paused programs only, MUST exclude dormant identities and custody-only accounts, and MUST alone govern `ActiveActorLimit`; create-active and activate transitions increment it only after full validation, while deactivation decrements it atomically. Closing either active or dormant identity decrements identity cardinality, releases a User slot or records Mutable System reopen lineage, and preserves sovereign balances.

Close precedence is checkpoint-scoped and deterministic: `WindowExpired` dominates all external/admission touch points; for unpaused User AAA admission, `BalanceExhausted` dominates `FeeBudgetExhausted`; stored nonce exhaustion prevents any further normal-cycle admission; after an admitted cycle, `ConsecutiveFailures` is the only post-failure terminal close and `AutoCloseNonceReached` is the only post-success terminal close. If a scheduler, post-cycle, or sweep-time close transaction rolls back, the actor MUST emit `CycleDeferred(CloseTransitionFailed)` and retry the stored terminal reason deterministically: scheduler/post-cycle paths remain durably queued before readiness or another normal cycle, while sweep cleanup retains its cursor before the actor and stops that pass so the next sweep retries the same candidate.

Before terminal state removal, runtime MUST enter close-tail execution for explicit and automatic close paths. The close tail uses the same task, condition, amount-resolution, error-policy, adapter, and weight-upper-bound semantics as the main plan. User close-tail admission derives `close_cycle_weight_upper`/`close_cycle_fee_upper`, reserves `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`, and builds a fresh `TriggerSnapshot`; System AAA uses the same execution semantics with zero User fee charging. Close-tail execution MUST NOT recurse into another close. If fee-native balance depletes or fee collection fails during admitted close execution, affected close steps MUST fail/skip observably while final closure still completes; whole-tail skip is not part of the current contract.

Close-tail contract matrix:

- Creation/activation: active input commits an explicit bounded close plan, which MAY be empty; mutable actors MAY replace it later
- Explicit close: control origin enters close tail inline before deletion, independent of mutability
- Automatic close: scheduler admits close tail only when bounded budget fits; otherwise closure defers
- Sweep close: sweep is a lifecycle touchpoint; it may enter terminal close but MUST NOT admit a normal cycle
- Fee depletion: per-step close failure/skip is observable and MUST NOT block later steps or final closure
- Nonce: close tail MUST NOT increment `cycle_nonce` or emit `CycleStarted`/`CycleSummary`
- Deletion: remove actor/readiness/index state, remove its authoritative future wakeup through the reverse pointer, clear User slot, preserve sovereign balances, emit `AaaClosed`
- Recovery: User reattaches with `create_user_aaa_at_slot`; System reopens with `reopen_system_aaa`

Create/close transitions MUST synchronize actor identity/program stores and `SovereignIndex`; deactivate/close MUST invalidate queue/readiness entries, and deterministic stale queue entries MUST be ignored at pop. Direct post-destruction balance-rescue extrinsics remain out of the stable contract.

### 2.5 Funding Batches

The bounded per-asset `funding_snapshots` map is the canonical baseline for `PercentageOfLastFunding` resolution (Section 5.3). Each `FundingBatch` exposes the armed `amount` plus checked-add pending funding for the next successful operation; `funding_tracked_assets` bounds the map and producer work.
Required behavior:

1. **Execution-Plan Scanning**: On creation or either plan update, runtime MUST scan BOTH plans and populate `funding_tracked_assets` with each ordinary spent `AssetId` using `PercentageOfLastFunding` and each Unstake `StakingOps::share_asset(position_asset)` used by that mode; validation MUST reject Unstake last-funding resolution when the adapter returns `None`. Updates MUST fully recompute tracked assets and prune batch state no longer tracked.
2. **Source Policy**: Each actor stores one bounded `FundingSourcePolicy`: `OwnerOnly`, `SignedAllowlist(BoundedSet<AccountId>)`, `RuntimePolicy`, or explicit `AnySource`. User AAA defaults to `OwnerOnly`; System AAA defaults to `RuntimePolicy`. `AnySource` accepts every verified producer provenance, not source-less ingress. Immutable actors fix policy at creation; Mutable actors may update it through the same class-specific control authority, without rewriting existing batches.
3. **Verified Provenance**: Supported producers MUST supply runtime-verified provenance independently of trigger matching: signed ingress identifies the debited signer, internal-protocol ingress identifies a typed runtime source, and XCM ingress identifies its converted origin/location class. The pallet MUST evaluate stored policy: `OwnerOnly`/`SignedAllowlist` compare the verified debited signer, `SignedAllowlist` remains bounded, `AnySource` still requires provenance, and only `RuntimePolicy` delegates `(aaa_id, owner, provenance)` to the runtime `FundingAuthority`, which MUST default deny absent an explicit actor/source authorization. Actor class determines the default policy; missing, unverified, or policy-rejected provenance is balance-only.
4. **Standard Funding Ingress**: The stable contract has no dedicated funding extrinsic. Every successful inbound transfer still credits sovereign balance; only a positive tracked transfer accepted by Items 2–3 may mutate `funding_snapshots`, while all other transfers remain balance-only donations. Funding authority and `OnAddressEvent` source/asset filters MUST be evaluated independently: neither acceptance nor rejection by one implies the result of the other.
5. **Bootstrap and Accumulation**: The first authoritative transfer with no batch entry MUST set `amount` so initial funding needs no empty cycle. Once armed, later authoritative transfers MUST checked-add into `pending_amount` and MUST NOT change `amount`; pending overflow MUST fail observably and transactionally roll back both the producer transfer and batch mutation rather than clamp or overwrite. Funding-event timestamps are not consensus state because promotion and amount resolution do not consume them.
6. **Frozen Resolution**: Normal-cycle and close-tail `PercentageOfLastFunding` MUST resolve only from the batch `amount`. Close-only execution MUST ignore pending, MUST NOT promote it, and terminal deletion removes batch state while sovereign balances remain in place.
7. **Successful Promotion**: After `CycleSummary` for a successful cycle as defined in Section 2.6, every nonzero `pending_amount` MUST replace `amount` and clear pending atomically; assets without pending retain their armed values. `AbortCycle`, Weight deferral, pause, breaker deferral, and any path without an admitted successful cycle MUST preserve armed and pending state unchanged.
8. Funding-batch mutation remains valid while paused but MUST NOT imply automatic pause/resume; preflight and notification for expired or closed actors MUST remain balance-only. `FundingUnavailable` remains a deterministic non-terminal outcome covering absent/zero armed state and armed-amount overspend, while an armed value remains valid until successful promotion or plan pruning.
9. `cycle_weight_upper` and `cycle_fee_upper` are run-plan cache fields that MUST be recomputed on create/update execution plan and MUST only affect admission/preflight efficiency, not functional execution semantics. Close-tail upper bounds (`close_cycle_weight_upper`, `close_cycle_fee_upper`) MUST also remain deterministically derivable from `on_close_execution_plan` on create/update, whether cached or recomputed, and MUST NOT alter functional task semantics.

### 2.6 Failure Tracking

1. A cycle is **successful** exactly when execution reaches plan completion without any failed step selecting `AbortCycle`; skipped steps and any number of failed `ContinueNextStep` steps still satisfy this predicate. `consecutive_failures` increments only when `AbortCycle` makes the predicate false; if `MaxConsecutiveFailures > 0`, the terminal cutoff is inclusive (`>=`).
2. `consecutive_failures` resets on successful cycle completion, and only a successful cycle may satisfy `auto_close_at_cycle_nonce`.
3. Deferrals MUST NOT increment `consecutive_failures`.
4. `update_execution_plan` (Mutable) MUST reset `consecutive_failures`.
5. `cycle_nonce` stores the number of admitted normal cycles and starts at `0`. When its stored value is below `u64::MAX`, admission MUST increment it before `CycleStarted`; therefore the first admitted cycle emits nonce `1`, and admission from `u64::MAX - 1` emits nonce `u64::MAX` and executes normally.
6. Deferred cycles MUST NOT increment nonce; `last_cycle_block` MUST update to `current_block` exactly with `CycleStarted`, not on completion or deferral.
7. A subsequent scheduler attempt with stored `cycle_nonce == u64::MAX` MUST NOT emit `CycleStarted`/`CycleSummary` or dispatch normal-plan steps: User AAA enters admitted close-tail closure with `CycleNonceExhausted`, while System AAA pauses with `PauseReason::CycleNonceExhausted`.

---

## 3. Adapters

All operations MUST go through typed adapters. The external host-runtime embedding checklist and adapter failure atomicity matrix are maintained in [AAA External Runtime Embedding Guide](./aaa.embedding.en.md) as the implementation-facing companion to this normative adapter contract.

### 3.1 AssetOps

```rust
trait AssetOps<AccountId, AssetId, Balance> {
    fn transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn balance(who: &AccountId, asset: AssetId) -> Balance;
    fn minimum_balance(asset: AssetId) -> Balance;
    fn can_deposit(who: &AccountId, asset: AssetId, amount: Balance) -> bool;
}
```

**Balance semantics**: `balance()` MUST return the adapter-visible immediately transferable balance for the asset before any AAA-local reservation is applied. For `FeeNativeAsset` this is runtime policy (typically `free_balance` after adapter-level locks/reserves/holds); for assets without hold semantics it may equal total balance. AAA then derives `spendable_fee_native` by subtracting transient `reserved_fee_remaining` from `FeeNativeAsset` `balance()` only; non-`FeeNativeAsset` balances are passed through unchanged for spendability checks.

`Mint` MUST be rejected for User AAA in both the run plan and close plan. Validation MUST occur at every plan-admission path (`create_*`, `update_execution_plan`, `update_on_close_execution_plan`, and any default close-plan injection); if a User plan contains `Mint`, the call MUST fail with `MintNotAllowedForUserAaa`.
`can_deposit`/`minimum_balance` are REQUIRED for ED-safe split-transfer normalization (Section 6.2).

### 3.2 DexOps

```rust
trait DexOps<AccountId, AssetId, Balance> {
    fn swap_exact_in(
        who: &AccountId,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_in: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, DispatchError>;
    fn swap_exact_out(
        who: &AccountId,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_out: Balance,
        max_amount_in: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, DispatchError>;
    fn add_liquidity(who: &AccountId, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance)
        -> Result<(Balance, Balance, Balance), DispatchError>;
    fn remove_liquidity(who: &AccountId, lp_asset: AssetId, lp_amount: Balance)
        -> Result<(Balance, Balance), DispatchError>;
}
```

Adapter contract:

1. Complexity MUST be O(1) or bounded O(K) with explicit `MaxK` constants.
2. Storage iteration (if any) MUST use canonical storage-key order.
3. Rounding behavior MUST be fixed and deterministic per method.
4. `SwapExactIn` MUST derive `min_out` from a caller-aware exact-input quote that includes runtime routing fees and selects by the same mechanism used for execution; `Perbill::zero()` accepts no deterioration from that executable quote.
5. `SwapExactOut` receives policy-derived `max_amount_in` plus `slippage_tolerance`, MUST derive a caller-aware required-input quote, MUST reject when the tolerance-adjusted bound exceeds `max_amount_in`, and MUST never debit more than `max_amount_in`.
6. Slippage/routing logic remains inside the DEX adapter; AAA supplies amount and spend-capacity bounds and handles `DispatchError` via `on_error`.

### 3.3 StakingOps

```rust
trait StakingOps<AccountId, AssetId, Balance> {
    fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), DispatchError>;
    fn share_balance(who: &AccountId, asset: AssetId) -> Balance;
    fn share_asset(asset: AssetId) -> Option<AssetId>;
}
```

AAA MUST NOT encode runtime-specific staking topology such as collator choice, nomination custody, receipt naming, or native liquid-staking mechanics in the task enum. `Unstake.asset` is a runtime-defined staking-position key: `PercentageOfCurrent`, `PercentageOfTrigger`, and `AllBalance` resolve through `share_balance`, while `PercentageOfLastFunding` resolves through `funding_snapshots[share_asset(asset)].amount` and MUST be rejected at plan validation when no transferable share asset exists. Runtime adapters MAY route native staking through chain-local primitives, but those semantics remain adapter policy outside the AAA pallet contract.

### 3.4 LiquidityDonationOps

```rust
trait LiquidityDonationOps<AccountId, AssetId, Balance> {
    fn donate_liquidity(
        who: &AccountId,
        asset_a: AssetId,
        asset_b: AssetId,
        amount: Balance,
        max_ratio_error: Perbill,
    ) -> Result<(Balance, Balance), DispatchError>;
}
```

AAA treats liquidity donation as an adapter-owned primitive rather than a DEOS-specific staking or AMM policy. The pallet resolves the declared `amount` against `asset_a`, passes `asset_a`, `asset_b`, `amount`, and `max_ratio_error` to the adapter, and records only the deterministic returned `(amount_a, amount_b)` in the success event. Pair-ratio checks, receipt suppression, reserve donation semantics, and any native-special-case routing remain runtime adapter policy.

Reusable AAA runtimes that do not need this capability MAY bind the no-op adapter, in which case `DonateLiquidity` tasks fail deterministically through ordinary `DispatchError` handling. Runtimes that expose `DonateLiquidity` in user-facing plan builders SHOULD describe the adapter's asset-pair and ratio semantics outside the AAA pallet contract.

### 3.5 Task Weight Contract

Runtime MUST expose deterministic worst-case bounds:

`fn weight_upper_bound(task: Task, params: TaskParams) -> Weight`

Requirements:

- State-independent for fixed params and bounded by configured `Max*`.
- A new core `Task` variant MUST represent a reusable economic primitive rather than runtime topology or product policy; prove that existing task composition plus an adapter cannot preserve the required atomicity/custody contract; define typed bounded parameters, amount-resolution and funding/donation sensitivity, adapter ownership, events/errors/`StepErrorPolicy` behavior, task-scoped rollback, a generated two-dimensional worst-case bound, production-budget admission evidence, semantic tests, and explicit SCALE discriminant/schema-version impact before merge. Runtime-specific behavior that fails this gate belongs in an adapter or actor graph, not the core enum.
- Always `>=` actual execution in both Weight dimensions, including adapter calls, storage proofs, fee collection, and emitted events.
- Full-cycle admission uses the sum of step bounds plus evaluation/execution fee-collection and cycle/lifecycle overhead.
- Task-level `weight_upper_bound` MUST include worst-case event emission cost for events produced by successful task execution.
- Runtime admission accounting MUST include deterministic step/cycle overhead for non-task events (`CycleStarted`, `StepSkipped`, `StepFailed`, `CycleSummary`, and lifecycle events emitted on terminal transitions).

Runtime SHOULD classify tasks by materially distinct worst-case work rather than merge adapters with different bounds:

| Bucket | Tasks |
| --- | --- |
| `SimpleAssetOp` | `Transfer`, `Burn`, `Mint` |
| `DexExactIn` | `SwapExactIn` |
| `DexExactOut` | `SwapExactOut`, including bounded quote search |
| `DexLiquidity` | `AddLiquidity`, `RemoveLiquidity`, `DonateLiquidity` |
| `Staking` | `Stake`, `Unstake` with runtime adapter bounds |
| `Fanout` | `SplitTransfer` (parameterized by `legs`) |

---

## 4. Economics

System AAA is exempt from User fee charging in this section. Every collected User fee MUST pass through one runtime-defined `FeeCollector` transaction that atomically transfers the full amount into the deposit-capable `FeeSink`; collection MUST NOT split by author, treasury, or downstream allocation policy. Opening creation MUST roll back if collection fails. Cycle-path collection failures MUST map deterministically to `StepFailed` and obey `StepErrorPolicy`, while close-tail failures MUST remain observable through `OnCloseStepFailed` without blocking later close steps or final closure.

### 4.1 Fee Model

Execution MUST follow this order:

1. **MinUserBalance Gate**
2. **Pre-flight Fee Admission** (`cycle_fee_upper`)
3. **Cycle Start / Fee Reservation**
4. For each step: charge evaluation fee → evaluate conditions → resolve task amount → if executable, charge execution fee → dispatch task.

For User AAA, insufficient pre-flight fee budget yields immediate `AaaClosed(FeeBudgetExhausted)`.

Per-step formulas:

- `eval_fee = StepBaseFee + ConditionReadFee × conditions.len()`
- `exec_fee_upper = WeightToFee(weight_upper_bound(task, params))`
- `cycle_fee_upper = Σ(eval_fee_i + exec_fee_upper_i)`
  Execution fee is charged once a step becomes executable, even if dispatch later fails; steps resolved to `Skipped` or `FundingUnavailable` do not incur execution fee, and their unused execution-fee reservation MUST be released before resolving later steps.

`StepBaseFee` and `ConditionReadFee` are charged before task dispatch and MUST be calibrated to economically cover non-executable paths (`StepSkipped`, `StepFailed`) that still consume reads/writes and emit events.

Close-tail admission uses the same formulas over `on_close_execution_plan`. Runtime MUST derive deterministic `close_cycle_weight_upper` and `close_cycle_fee_upper` from the close plan using the same task upper bounds and the same non-task observability overhead relevant to close-time execution.

### 4.2 No-Rent Policy

AAA uses no recurring rent mechanism. Long-horizon deferred scenarios remain valid within `MaxExecutionDelayBlocks` when lifecycle and fee-admission checks pass at execution time.

### 4.3 Fee Reservation

During cycle or admitted close-tail execution, runtime MUST keep `reserved_fee_remaining` and compute fee-native spend capacity as:

`spendable_fee_native = max(fee_native_balance - reserved_fee_remaining, 0)`

`reserved_fee_remaining` is a transient execution-context variable. It MUST NOT be persisted in `AaaInstance` or storage.

Reservation rules:

1. On admitted cycle start, initialize `reserved_fee_remaining = cycle_fee_upper`; on admitted User close-tail start, initialize `reserved_fee_remaining = min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`.
2. Every successful evaluation/execution fee charge MUST decrement `reserved_fee_remaining` by the charged amount.
3. All `FeeNativeAsset` spend paths MUST resolve amounts from `spendable_fee_native`, never from `balance()` alone.
4. On cycle or close-tail exit, unspent reserve is released by discarding the transient context; charged fees are NOT refunded.
5. Post-dispatch fee refund by actual consumed task weight is deliberately out of scope: AAA charges deterministic upper-bound execution fees per executable step for predictable admission economics.

### 4.4 Opening Fee

`create_user_aaa` MUST charge `AaaCreationFee` in `FeeNativeAsset` through the same atomic `FeeCollector` used for cycle fees; the opening fee is non-refundable (never returned on `close_aaa`), creation MUST fail and roll back if collection fails or the payer cannot cover it plus normal transaction fees (`InsufficientFee`), and `create_system_aaa` is exempt.

### 4.5 Close-Tail Admission and Forecasting

`on_close_execution_plan` is the terminal tail of the same deterministic execution pipeline, not a fee-free cleanup exception.

1. Runtime MUST derive `close_cycle_weight_upper` and `close_cycle_fee_upper` from `on_close_execution_plan` using the same task bounds, fee formulas, and close-time observability overhead as normal cycle admission.
2. Runtime MUST treat explicit and automatic closes the same at close-tail entry: build a fresh `TriggerSnapshot`, initialize User close reservation as `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`, and reuse zero User fee reservation for System AAA.
3. Scheduler-driven automatic closes MUST reserve enough dispatch budget early enough to admit the close tail; if bounded `on_idle` budget cannot fit that tail yet, runtime MUST defer closure rather than converting the current line back into whole-tail skip semantics.
4. Close execution MUST emit `OnCloseStepSkipped` for condition, resolution, and funding skips and classify failures as evaluation fee, execution fee, condition, resolution, or adapter. A User fee collection call MUST NOT debit more than `reserved_fee_remaining`; insufficient reserve or balance fails that step observably, while deterministic closure continues without retry loops.
5. Close finalization MUST occur exactly once after admitted close-tail completion.
6. Actors and tooling SHOULD forecast close viability with `predicted_fee_native_residual_after_close = fee_native_balance_before_close - close_cycle_fee_upper - close_task_fee_native_spend_upper`; values `<= 0` indicate close-tail attrition risk but MUST NOT block explicit owner/governance close.
---
## 5. Execution

### 5.1 Step

```rust
struct Step {
    conditions: BoundedVec<Condition, MaxConditionsPerStep>,
    task: Task,
    on_error: StepErrorPolicy,
}
```

### 5.2 Conditions

`Condition` type reference is normative in Section 12.1.

- Conditions are AND-composed.
- Empty condition list = unconditional step.
- Evaluation errors are fail-closed (`StepFailed`).
- Balance conditions evaluate against **spendable balance** (adapter-visible balance minus `FeeNativeAsset` reserved fee budget where applicable), not against `balance()` alone. This ensures conditions and amount resolution share a unified view of available funds.

### 5.3 Amount Resolution

`AmountResolution` type reference is normative in Section 12.1.

Semantics:

1. Under `PreserveSpend`, `preservable_current = max(spendable_current - minimum_balance(asset), 0)`; `Fixed` and snapshot-derived amounts MUST NOT exceed that capacity, `PercentageOfCurrent` uses it as the percentage base, and `AllBalance` resolves to it without clamping another resolution mode.
2. `PercentageOfTrigger` uses the cycle-start snapshot (Section 5.4), then applies the task policy's current-capacity check.
3. `PercentageOfLastFunding` uses `funding_snapshots[asset].amount` for the task's resolution surface (Section 2.5), then applies the current-capacity check; pending funding never changes an already armed amount.
4. Under `ExpendableSpend` and `Mint`, `AllBalance` resolves to full `spendable_current`; Unstake resolves share amounts through `StakingOps` and permits full share withdrawal.
5. Resolution outcomes are deterministic: `Resolved(amount)`, `Skipped` (e.g. tiny percentage rounds to zero), or `FundingUnavailable`; resolution MUST NOT silently clamp a requested amount to policy capacity.

Resolution policies — runtime MUST apply one per task:

- `PreserveSpend`: `Transfer`, `SplitTransfer`, `SwapExactIn`, `AddLiquidity`, `RemoveLiquidity`, `Stake`, `DonateLiquidity`; subtract ED and require a spendable source; `DonateLiquidity` resolves only its declared `asset_a` amount, while the adapter derives `asset_b`
- `Mint`: `Mint`, `SwapExactOut`; no ED subtraction or spendability requirement; for `SwapExactOut`, this policy applies only to target output, while the `DexOps` capacity contract (Section 3.2) and task input rule (Section 6.1) define the input-capacity bound
- `ExpendableSpend`: `Burn`, using full spendable balance; `ShareSpend`: `Unstake`, using adapter-visible shares with full withdrawal allowed; multi-amount tasks MUST resolve every field before dispatch and apply outcome precedence `FundingUnavailable > Skipped > Executable` independent of field order

For `SwapExactOut`, `Mint` policy applies only to target output resolution. AAA MUST derive `max_amount_in` from the `asset_in` policy capacity, including `reserved_fee_remaining` and minimum-balance preservation for `FeeNativeAsset`, and the DEX adapter MUST enforce that bound atomically.

Execution mapping:

- `Resolved(amount)`: task executes normally
- `Skipped`: emit `StepSkipped { reason: ResolutionSkipped }`; non-failing and does not increment `consecutive_failures`
- `FundingUnavailable`: emit `StepSkipped { reason: FundingUnavailable }`; non-terminal for both actor classes (Section 2.4)
- Conditions not met: emit `StepSkipped { reason: ConditionsNotMet }`

### 5.4 Trigger Snapshot

`PercentageOfTrigger` resolves against a frozen balance snapshot taken once at cycle start, or once again at close entry for `on_close_execution_plan`. This eliminates compound-percentage effects across multi-step execution plans.

Example (sovereign holds 1000 Native, 3-step execution plan):

- Step 0: transfer `PercentageOfTrigger(10%)`; resolves `100`; balance → 900
- Step 1: transfer `PercentageOfTrigger(10%)`; resolves `100`; balance → 800
- Step 2: swap `PercentageOfTrigger(50%)`; resolves `500`; balance → 300

Construction rules:

1. At admitted cycle start, after fee reservation (Section 4.3), or at admitted close-tail entry after close-tail fee reservation, runtime MUST build a transient `TriggerSnapshot` keyed by typed ordinary-asset or staking-share resolution surface.
2. Scan execution-plan steps for all `PercentageOfTrigger` references and collect unique typed resolution surfaces: ordinary assets and Unstake staking-position shares.
3. For ordinary assets, snapshot `spendable_fee_native` or `AssetOps::balance`; for Unstake, snapshot `StakingOps::share_balance(sovereign, position_asset)` under a distinct typed key.
4. `TriggerSnapshot` is transient per-cycle execution context alongside `reserved_fee_remaining`; MUST NOT be persisted; released on cycle or close-tail exit.
5. If a step references `PercentageOfTrigger` for an asset absent from the snapshot, resolution MUST return `Skipped`.

Execution-plan scan (step 2) is bounded by `MaxSteps` and incurs no storage I/O.

### 5.5 Error Policies and Atomicity

```rust
enum StepErrorPolicy {
    AbortCycle,
    ContinueNextStep,
}
```

- `AbortCycle`: stop immediately; increment `consecutive_failures`.
- `ContinueNextStep`: skip failed step and continue.
- `PauseActor` MUST NOT be part of stable `StepErrorPolicy`; step failure can abort or continue only, while actor pause remains an explicit lifecycle transition (`pause_aaa`) or the dedicated nonce-exhaustion safety transition.

Atomicity:

- **Task-level**: atomic.
- **Execution-plan-level**: non-atomic (previous successful steps persist).

Task atomicity rules:

1. Every executable task MUST run inside a task-scoped transactional boundary owned by AAA or by an adapter with equivalent commit/rollback semantics.
2. Multi-op tasks (e.g. `SplitTransfer`) MUST execute within that boundary across the full normalized operation set.
3. If any task or sub-operation fails, the task MUST revert all prior task-local effects. Partial task effects MUST NOT persist, including adapter mutations that fail after an intermediate burn/transfer.

If an early step mutates asset composition and a later step fails, post-mutation balances remain on the sovereign account. When using `ContinueNextStep` after mutating tasks (`SwapExactIn`, `SwapExactOut`, liquidity ops), execution-plan authors and UIs SHOULD guard downstream steps with explicit balance conditions. Execution-plan simulation is off-chain only (RPC dry-run, fork replay).

---

## 6. Tasks

### 6.1 Task Set and Parameters

- `Transfer`: single asset transfer
- `SplitTransfer`: atomic bounded fan-out (Section 6.2)
- `Burn`: asset burn
- `Mint`: asset mint (System AAA only)
- `SwapExactIn`: DEX exact-in with `Perbill` slippage tolerance
- `SwapExactOut`: DEX exact-out target with deterministic input resolution
- `AddLiquidity`: provide liquidity
- `RemoveLiquidity`: withdraw liquidity
- `Stake`: deposit declared asset into staking adapter; native support uses the runtime's chosen `AssetId`
- `DonateLiquidity`: donate value into a pair without minting LP; adapters own pair-specific balancing
- `Unstake`: withdraw shares from staking pool

`SwapExactIn` parameter contract:

```rust
SwapExactIn {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

`slippage_tolerance` is passed directly to `DexOps`; the adapter obtains a caller-aware executable quote after routing fees and computes `min_out = (1 - slippage_tolerance) × quoted_recipient_output`. `Perbill::zero()` requires that quoted output, `Perbill::one()` accepts any output, and unavailable routes fail through `DispatchError` handled by `on_error`.

`SwapExactOut` parameter contract:

```rust
SwapExactOut {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

For `SwapExactOut`, AAA passes policy-derived input capacity and the adapter MUST resolve caller-aware required input deterministically, derive `quoted_max_in = (1 + slippage_tolerance) × quote_required_in`, require `quoted_max_in <= max_amount_in`, and never debit more than either bound.

### 6.2 SplitTransfer

```rust
struct SplitTransfer {
    asset: AssetId,
    amount: AmountResolution<Balance>,
    legs: BoundedVec<SplitLeg, MaxSplitTransferLegs>,
}
```

`SplitLeg` type reference is normative in Section 12.1.

Validation:

1. `2 <= legs.len() <= MaxSplitTransferLegs`
2. No zero-share legs (`share > 0`)
3. No duplicate recipients
4. `sum(share_i) <= Perbill::one()`; if exceeded, MUST fail with `InvalidSplitTransfer`

Allocation:

- `leg_i = floor(total × share_i)`
- `distributed = Σ(leg_i)`
- `retained = total - distributed`

Remainder semantics:

- `sum(share_i)` MAY be `< 100%`.
- Any undistributed part and rounding dust MUST remain on the AAA sovereign balance.
- Runtime MUST NOT auto-route retained remainder to any recipient.

ED safety:

1. Before dispatching transfers, runtime MUST run deterministic leg normalization.
2. For each leg with `leg_i > 0`, if `AssetOps::can_deposit(to_i, asset, leg_i) == false`, that leg MUST be skipped and its amount added to `retained`.
3. Final `retained` amount MUST remain on the AAA sovereign balance.

The whole fan-out remains task-atomic for the final normalized transfer set.

### 6.3 Task Contract

Each task MUST define: validation rules, deterministic error surface, deterministic `weight_upper_bound`, and explicit adapter side effects. Tasks MUST NOT dispatch arbitrary extrinsics.

---

## 7. Triggers

### 7.1 Deterministic Timer

```rust
enum Trigger<AccountId, AssetId> {
    Timer { every_blocks: u32 },
    OnAddressEvent {
        source_filter: SourceFilter<AccountId>,
        asset_filter: AssetFilter<AssetId>,
    },
    Manual,
}
```

`Schedule` type reference is normative in Section 12.1.

Schedule cooldown rules:

1. `cooldown_blocks` MUST apply to all trigger classes (`Timer`, `OnAddressEvent`, `Manual`) after the first admitted cycle.
2. The first admission, identified by stored `cycle_nonce == 0` before its admission increment, MUST NOT be blocked by cooldown.
3. One canonical saturating calculation MUST derive effective eligibility as the maximum of applicable `last_cycle_block + cooldown_blocks`, timer-cadence-plus-jitter, and `schedule_window.start` terms; cooldown is omitted before the first admitted cycle, while first delayed-timer eligibility anchors to actor creation and an every-block timer is immediately eligible.
4. `last_cycle_block` MUST be updated at admitted cycle start (`CycleStarted`) and therefore records the admitted-run clock rather than completion or pre-admission deferral.
5. A pending Manual or AddressEvent signal MUST omit the timer term even when Manual explicitly triggers a Timer actor; cooldown and window terms still gate admission.
6. `manual_trigger` MAY set `manual_trigger_pending`, but cooldown MUST still gate admission.

Timer rules:

1. `every_blocks` MUST satisfy `0 < every_blocks <= MaxExecutionDelayBlocks`; otherwise fail with `ExecutionDelayTooLong`.
2. Timer cadence is deterministic and exposes no probability or entropy input.
3. Effective timer eligibility at or before the next block MUST use queue self-continuation (`NextQueue`); later eligibility MUST persist exactly one `WakeupIndex` entry, including `every_blocks = 1` when cooldown or window delay dominates.
4. Timer rearming, skipped-readiness preservation, and readiness checks MUST use the same effective-eligibility calculation rather than independent cadence/cooldown/window branches.
5. Deterministic anti-storm jitter SHOULD be applied for delayed timers (`every_blocks > 1`):
   `jitter_window = min(every_blocks / 4, MaxTimerJitterBlocks)`
   `jitter = Blake2_256(aaa_id) % jitter_window` when `jitter_window > 0`, else `0`; validation MUST require `every_blocks + max(jitter_window - 1, 0) <= MaxExecutionDelayBlocks`
   `timer_eligible_at = admitted_run_anchor + every_blocks + jitter`.

A future probabilistic trigger requires a separate append-only trigger variant plus a concrete deterministic, financially secure runtime entropy contract; it MUST NOT reintroduce optional probability into `Timer` or hash-fallback sampling.

### 7.2 OnAddressEvent

```rust
// AddressEventInbox contains aaa_id iff one signal is pending.
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

Inbox model:

1. `AddressEventInbox` is a per-AAA presence latch, not an event queue; multiple matched events coalesce into one pending signal without consensus-state generation or timestamp metadata.
2. A matched inbound balance-increase event inserts the actor key idempotently; the key is removed only when the signal is consumed or actor lifecycle cleanup invalidates it.
3. Coalescing is signal-level: one admitted cycle may consume balances accumulated from multiple matched events since the previous inbox consumption.

Rules:

- `SourceFilter::Whitelist` and `AssetFilter::Whitelist` MUST be non-empty and bounded by `MaxWhitelistSize`.
- Events without a concrete source account identifier MUST match only `SourceFilter::Any`.
- Scheduler readiness for this trigger MUST be `true` iff the actor key exists in `AddressEventInbox`.
- When a cycle starts for an actor with `OnAddressEvent`, the inbox entry MUST be consumed atomically.
- If a new matched event arrives after consumption, the actor MUST become ready again on subsequent scheduler passes.

Ingress contract:

1. Runtime ingress to `OnAddressEvent` MUST go through a runtime-configured adapter interface (`AddressEventIngress` or equivalent) that ultimately invokes `notify_address_event*`.
2. Ingress strategy SHOULD be submit-first: producer paths with explicit hook points (AAA asset ops, TMC/router routing paths, XCM transactor paths) SHOULD submit directly through the adapter at successful transfer/mint completion; generic top-level calls without pallet callbacks MAY use a weight-charging post-dispatch transaction extension as their producer-owned hook.
3. Runtime event-vector scanning MUST NOT serve as a supported producer ingress path because a bounded prefix cannot retain events beyond the scan cap; generic top-level calls without pallet callbacks MUST use the producer-owned transaction extension or another weighted submit adapter.
4. Producer paths MUST NOT mutate `AddressEventInbox` or `funding_snapshots` directly.
5. Source and asset filters MUST be evaluated in the same state transition as the inbox update.
6. Source invariant: when a concrete sender is available, ingress MUST preserve it exactly; `source = None` is valid only for inherently source-less paths.
7. Ingress identity invariant: runtime MUST process each producer event position exactly once without content-based coalescing; distinct same-block transfers with identical actor, asset, amount, and provenance MUST remain distinct. Accepted durable enqueue MUST reserve/apply funding exactly once, while its later drain delivers only the trigger/inbox effect and MUST NOT replay funding.
8. Funding-batch behavior for inbound ingress is normative (Section 2.5) and MUST remain independent from trigger-filter matching.
9. Boundedness invariant: every supported producer MUST submit directly or transactionally persist the event plus any accepted funding mutation in the producer-owned bounded overflow queue. The originating transaction MUST propagate `preflight_funding_event` and fallible `notify_address_event*` failures, and MUST treat a rejected durable enqueue as failure; queue saturation or funding overflow MUST roll back value movement. Scanner-only events are outside the supported ingress contract.
10. Weight for direct funding/inbox mutation is paid by the originating transfer/mint path; bounded overflow enqueue pays funding mutation synchronously and reserves both Weight dimensions for trigger-only drain work in `on_idle`. Runtime integration evidence MUST exercise matched, unmatched, failed, and post-dispatch refund behavior through the executive extrinsic pipeline.

### 7.3 Manual Trigger

`manual_trigger` bypasses schedule timing only. It MUST NOT bypass admission or fee checks.

1. Calling `manual_trigger` on an eligible unpaused actor MUST set `manual_trigger_pending = true` and perform a bounded enqueue/schedule request; calling it on a paused actor MUST fail with `AaaPaused`; calling it on System Immutable AAA MUST fail with `ImmutableAaa`.
2. `manual_trigger_pending` MUST be cleared exactly when a cycle is admitted and `CycleStarted` is emitted.
3. Deferrals MUST NOT clear `manual_trigger_pending`.
4. If the actor closes before admission, the flag is removed with actor state deletion.
5. If the actor is paused after the flag is set, `manual_trigger_pending` MUST persist across `pause_aaa` / `resume_aaa`; resume MUST re-enqueue a pending actor, and cooldown/window misses MUST schedule its earliest bounded eligibility without requiring another external signal.

### 7.4 Schedule Window

```rust
struct ScheduleWindow<BlockNumber> {
    start: BlockNumber,
    end: BlockNumber,
}
```

Validation:

1. `end > start`
2. `end - start >= MinWindowLength`
3. `saturating_sub(start, current_block) <= MaxExecutionDelayBlocks`; otherwise fail with `ExecutionDelayTooLong`

Semantics:

- `current_block < start`: not ready.
- `start <= current_block <= end`: eligible.
- `current_block > end`: lifecycle closure is handled per Section 2.4 (`WindowExpired` across lifecycle touch points).

Deferred-horizon contract:

- `MaxExecutionDelayBlocks` MUST represent exactly ten years in blocks for the runtime target block time.
- At creation and `update_schedule`, runtime MUST ensure first eligible execution is not delayed beyond `current_block + MaxExecutionDelayBlocks`.
- For `ScheduleWindow`, this bound is enforced via `start`.
- For `Timer`, this bound is enforced via `every_blocks` (Section 7.1).

---

## 8. Scheduler

The AAA runtime is a **deterministic event-driven actor runtime**. Actors are never polled globally; explicit Timer, AddressEvent, and Manual signals wake them. Asset ingress can function as a trigger-message, and larger workflows may emerge from actor graphs, but all work flows through one paged active FIFO plus a temporal wakeup layer and complete-operation admission.

### 8.1 Architecture: Two-Layer Scheduler

1. **Logical Active FIFO**: `QueueHead <= QueueTail` defines one monotonic ticket interval. Tickets are `u64`, never reused during active chain-state lifetime, and MUST fail closed rather than wrap. At scheduler-pass start, `block_cutoff = QueueTail`; only entries with `ticket < block_cutoff` may be considered. Enqueues during that pass receive later tickets and belong to a future block.
2. **Paged Physical Storage**: `QueuePages[page_id]` stores bounded consecutive entries, with the ticket derived from page and slot unless production-Wasm evidence justifies encoding it. The scheduler may stop mid-page or traverse many pages in one block; it persists the exact next head and deletes only fully consumed pages. `QueuePageSize` is I/O granularity, not throughput or execution capacity.
3. **Actor-Local Membership**: `ActorHot.queue_ticket` is the actor's sole live queue membership. An entry is live only when its ticket equals that field; otherwise it is a tombstone. Enqueue coalesces while a live ticket exists. Cancellation, close, pause, dormancy, and replacement invalidate actor-local state without scanning pages.
4. **Queue Continuation**: Cadence `every_blocks <= 1` re-admits actors through a new ticket beyond the captured cutoff rather than timer indexing.
5. **Temporal Wakeup Layer (`WakeupIndex` + `MinWakeupBlock`)**: Delayed timers (`every_blocks > 1`) use canonical bounded wakeup storage plus an actor-keyed live pointer; dormant, closed, or missing actors drop lazily when reached.
6. **Rejected Physical Extremes**: Production MUST use neither `StorageValue<BoundedRingBuffer<_, MaxQueueLength>>` nor `StorageMap<QueueTicket, QueueEntry>`. Algorithmic O(1) does not imply bounded physical trie I/O: the former inherits maximum-value decode/encode/proof behavior, while the latter pays one trie key and proof path per entry. A bounded intermediate page size amortizes trie overhead without coupling each touch to 10,000-entry capacity.

For every block `B` and actor `A`, the consensus invariant is:

```text
executions(A, B) <= 1
```

Signals or funding arriving after A executes may update actor-local pending state, but any resulting ticket is at or beyond the captured cutoff. Recursive self-enqueue and circular `A -> B -> A` graphs therefore cannot execute A twice in one pass. Future retry or continuation work MUST obey the same invariant.

### 8.2 Execution Flow

Each block MUST use a two-dimensional `WeightMeter` to admit ingress, wakeup, queue inspection, close, and cycle operations only when each complete next operation fits. The loop considers FIFO entries below `block_cutoff` until RefTime or ProofSize cannot admit the next required operation, the successful-execution count reaches configurable `MaxExecutionsPerBlock`, or an independently justified scan ceiling is reached. `MaxExecutionsPerBlock` is a defense-in-depth ceiling, MUST count only successfully started/completed execution attempts under the final event contract, and MUST NOT be multiplied into a global worst-case reservation. Stale, paused, invalid, skipped, or tombstoned entries do not consume it. The reference runtime permits up to 1,000 actor executions per block, subject to complete per-actor admission within remaining RefTime and ProofSize; this is not a throughput guarantee.

### 8.3 Scheduler Liveness Matrix

- Queue carry-over: deferred, leftover, and execution-created late enqueues persist in deterministic FIFO order and MUST be revalidated at pop. A ready live head that fails admission only because the remaining block budget cannot fit its complete unit MUST retain its queue position and become the first candidate next block; carry-over MUST NOT move it behind a later entry or assign it a new FIFO identity.
- Timer due: delayed wakeup moves to active queue; actor wakeup pointer clears when drained
- AddressEvent matched: set/keep inbox latch; enqueue best effort; overflow MUST NOT clear the latch
- Manual trigger: set flag and enqueue/schedule; deferral preserves flag until admitted cycle start
- Queue/wakeup full: spill deterministically; if no bucket fits, emit drop and retain source latch when any
- Paused/cooldown/pre-window actor: pop preserves manual/inbox latches and MUST retain or schedule one future eligibility entry; resume re-enqueues pending non-timer signals
- Dormant/closed/missing actor: stale queue/wakeup entries are ignored; deactivate/close removes canonical readiness/pointers
- Window expired at touch/pop: enter terminal close before normal mutation or execution
- Breaker active: bounded housekeeping may continue; normal cycles and scheduler-owned automatic close-tail admission MUST defer without cycle/close-tail events; explicit lifecycle touchpoints and sweep-time terminal close remain allowed

### 8.4 Enqueue Deduplication, Budget, and Fairness

1. An actor MUST own at most one live queue ticket and one live future wakeup. Repeated same-block signals, transfers, internal notifications, or funding events coalesce into actor-local pending state. Physical occupancy includes tombstones and remains bounded by unconsumed capacity; User-controlled enqueue/invalidate churn MUST be fee-accounted or independently rate-limited.
2. Head cleanup MUST retain sufficient admission to skip stale entries and reclaim fully consumed pages even under queue saturation. A live actor cannot append another entry while its ticket remains live. Page boundaries MUST NOT alter FIFO order or prevent scanning/execution from continuing into a later page.
3. Wakeup overflow probes `requested_block..requested_block + MaxSpilloverBlocks`, then emits `WakeupScheduleDropped`, increments `WakeupScheduleDrops`, and persists `WakeupRetryPending[aaa_id]` if no bucket fits. A bounded canonical-key pass retries live actors without a new external signal and clears each marker only after scheduling succeeds or the actor closes.
4. Runtime MUST reserve at least `MinOnIdleReservePct` for `on_idle`; before storage access, the hook MUST reserve its generated fixed base. Subsequent two-dimensional admission charges ingress, wakeups, each touched queue page and inspected entry, actor loading/admission, sweeps, cycle work, and complete close cleanup before the operation begins.
5. Implementations MUST distinguish entries scanned, actors loaded, actors admitted, actors executed, tombstones skipped, actors deferred, pages touched, and pages deleted. `MaxExecutionsPerBlock` MUST NOT serve as a physical scan limit. A separate `MaxQueueEntriesScannedPerBlock` MAY protect malformed/tombstone-heavy state only when benchmark evidence justifies it.
6. If either Weight dimension, the successful-execution ceiling, or a justified independent scan ceiling is reached, no additional inadmissible operation may start. A ready live head that fails only because its complete bound does not fit MUST retain its ticket and remain the first candidate next block. Under documented recurring-budget scenarios, the fairness SLO is starvation-free execution with nonce spread `<= 3`; this is not unconditional under zero or insufficient `on_idle` budget.

### 8.5 Sweep

1. `permissionless_sweep` and `permissionless_sweep_many` are lifecycle touchpoints only: they evaluate terminal liveness immediately and MUST NOT enqueue, admit, or execute normal cycles.
2. Breaker state MUST NOT block sweep-time liveness evaluation or terminal closure; if an actor remains alive, the call returns without queue mutation.
3. `SweepCursor` iteration and batch accounting MUST tolerate missing/closed `AaaId` entries and continue without aborting traversal.

### 8.6 Starvation Safeguard

Because actors are never globally polled, the protocol relies on the Bounded Double-Buffer plus explicit starvation telemetry to guarantee forward progress:

1. `MinWakeupBlock` MUST advance across overdue `WakeupIndex` work in bounded passes: one `on_idle` call processes at most `MaxWakeupsPerBlock` actor entries and at most that many block cursors, preserves a partially drained bucket or an unadmitted bucket at its current cursor, and resumes sparse-gap catch-up in later blocks.
2. Scheduled execution (timer or event) MUST roll over across saturated blocks through queue carry-over and wakeup spillover; if bounded wakeup spillover is exhausted, runtime MUST surface the incident via `WakeupScheduleDropped` and `WakeupScheduleDrops`. Closing an actor MUST use `ScheduledWakeupBlock` to remove its authoritative `WakeupIndex` entry in one bucket-bounded mutation; malformed orphan entries without a reverse pointer remain lazy-drop compatibility state only.
3. After the fixed hook base has been admitted, `IdleStarvationBlocks` MUST increment only when the breaker is inactive and either Weight dimension of the remaining `on_idle` budget after bounded housekeeping is zero; a budget too small for the base performs no telemetry work.
4. `IdleStarvationBlocks` MUST reset to zero as soon as both post-housekeeping Weight dimensions remain positive, including blocks where no actor is ready.
5. `IdleStarvationDetected` MUST emit exactly once on threshold crossing and MUST NOT repeat on every subsequent starved block.
6. Starvation telemetry is observability-only; it MUST NOT trigger emergency cycle execution or any alternate scheduler path.

---

## 9. Runtime Hooks

### 9.1 `on_initialize`

- MUST remain bounded and deterministic.
- MUST NOT dispatch AAA cycles.
- MAY do minimal bookkeeping.

### 9.2 `on_idle`

- MUST reserve the generated fixed hook base before any storage access, then meter bounded housekeeping (address-event ingress + zombie sweep) and skip any next unit that does not fit both remaining Weight dimensions.
- With breaker inactive: execute only fully admitted cycles using the remaining two-dimensional budget after housekeeping.
- With breaker active: skip cycle execution and run only fully metered housekeeping.
- MAY perform bounded lazy readiness/inbox transitions only after reserving their complete weight.
- MUST run the `IdleStarvationBlocks` state machine from Section 8.6 after bounded housekeeping determines the remaining execution budget.
- MUST NOT contain unbounded or unmetered loops.

---

## 10. Extrinsics

### 10.1 Owner / Control Extrinsics

- `create_user_aaa(mutability, program)`: create active or dormant actor at the lowest free owner slot; `program` is the complete typed input from Section 2.4
- `create_user_aaa_at_slot(owner_slot, mutability, program)`: exact recovery slot with the same complete active/dormant input
- `activate_aaa(aaa_id, active_program)`: atomically validate and install a complete program on a Mutable dormant actor
- `deactivate_aaa(aaa_id)`: atomically remove a Mutable actor's program and scheduler/funding state while preserving identity, slot, and balances
- `pause_aaa(aaa_id)`: pause actor (Mutable only)
- `resume_aaa(aaa_id)`: resume actor (Mutable only)
- `manual_trigger(aaa_id)`: set manual trigger flag
- `close_aaa(aaa_id)`: owner-initiated close, destruction in place
- `update_schedule(aaa_id, schedule, schedule_window)`: update schedule/window (Mutable only)
- `update_execution_plan(aaa_id, execution_plan)`: replace run plan and reset `consecutive_failures` (Mutable)
- `update_funding_source_policy(aaa_id, policy)`: replace bounded funding authority policy without rewriting existing batches (Mutable only)
- `set_auto_close_at_cycle_nonce(aaa_id, target)`: set/shorten/extend a cycle lease target or clear it with `None`; `Some(target)` requires `cycle_nonce < target <= cycle_nonce + MaxAutoCloseNonceHorizon`
- `increment_auto_close_nonce(aaa_id, by)`: extend from the existing target, or from current `cycle_nonce` when unset; require `by > 0`, checked addition, and a resulting target within `MaxAutoCloseNonceHorizon` of current nonce
- `update_on_close_execution_plan(aaa_id, on_close_execution_plan)`: replace close-time plan (Mutable only)

`execution_plan` is the normative term for the run-step vector.
`activate_aaa`, `deactivate_aaa`, `pause_aaa`, `resume_aaa`, `manual_trigger`, `close_aaa`, `update_schedule`, `update_execution_plan`, `update_funding_source_policy`, `set_auto_close_at_cycle_nonce`, `increment_auto_close_nonce`, and `update_on_close_execution_plan` share the same control gate: signed owner for both actor types, plus governance origin for System AAA only; governance MUST NOT control User AAA through this path. After origin passes, System Immutable actors MUST reject these control paths with `ImmutableAaa`. Every extrinsic that can close inline after lifecycle inspection MUST declare a FRAME dispatch weight that component-wise upper-bounds the maximum User/System close plan and complete terminal cleanup; no runtime "remaining dispatch weight" preflight substitutes for transaction admission.

`create_user_aaa` MUST pay normal transaction fees, charge `AaaCreationFee` through the atomic runtime `FeeCollector` (Section 4.4), and enforce the deferred-horizon cap (Section 7.4).

Active creation and `activate_aaa` MUST fail with `ActiveAaaCapacityExceeded` when active program count reaches `ActiveActorLimit`. Every creation or reopen MUST fail with `ActorIdentityCapacityExceeded` when active plus dormant identity count reaches `MaxActorIdentities`. Capacity checks and limit updates MUST use transactionally maintained O(1) counts rather than map iteration.

### 10.2 Governance Extrinsics

- `create_system_aaa(mutability, program)`: create a Mutable active/dormant or Immutable active System AAA
- `reopen_system_aaa(aaa_id, mutability, program)`: reopen a closed Mutable System AAA as Mutable active or dormant at the same `aaa_id`
- `set_global_circuit_breaker(paused: bool)`: global scheduler stop/resume
- `set_active_actor_limit(new_limit: u32)`: operational cap update (`0 < new_limit <= min(MaxActiveActors, MaxQueueLength)`)

`create_system_aaa(mutability, ...)` MUST allocate the fresh `aaa_id = NextAaaId`. `reopen_system_aaa(aaa_id, mutability, ...)` is the only stable explicit-id governance path for closed Mutable System AAA, MUST recreate it as Mutable, and MUST preserve deterministic sovereign re-derivation without rewinding `NextAaaId`; detailed id/occupancy rules are defined in Section 2.3.

### 10.3 Tooling Extrinsics

| Extrinsic | Description |
| --- | --- |
| `permissionless_sweep(aaa_id)` | Force lifecycle evaluation for one actor (REQUIRED) |
| `permissionless_sweep_many(ids)` | Bounded batch lifecycle evaluation (`len <= MaxSweepPerBlock`) |

### 10.4 Circuit Breaker

When breaker is active:

1. Scheduler MUST stop admitted cycle execution and scheduler-owned automatic close-tail admission; bounded housekeeping and queue/inbox/wakeup bookkeeping MAY continue, while ready actors and terminal conditions remain pending until the breaker clears or an explicit lifecycle/sweep path closes them.
2. Creation extrinsics MUST fail with `GlobalCircuitBreakerActive`.
3. Ordinary inbound transfers, `manual_trigger`, `close_aaa`, `permissionless_sweep`, and `permissionless_sweep_many` MUST remain operational; queued work executes only after breaker clears.

---

## 11. Observability

### 11.1 Events

```rust
AaaActivated { aaa_id }
AaaClosed { aaa_id, reason: CloseReason }
AaaCreated { aaa_id, owner, actor_class, sovereign_account, lifecycle }
AaaDeactivated { aaa_id }
AaaPaused { aaa_id, reason: PauseReason }
AaaResumed { aaa_id }
ActiveActorLimitSet { old_limit: u32, new_limit: u32 }
AutoCloseNonceIncremented { aaa_id, old_target: Option<u64>, new_target: u64, by: u64 }
AutoCloseNonceSet { aaa_id, target: Option<u64> }
BurnExecuted { aaa_id, asset, amount }
CycleDeferred { aaa_id, reason: DeferReason }
CycleStarted { aaa_id, cycle_nonce }
CycleSummary { aaa_id, cycle_nonce, executed_steps, skipped_conditions, skipped_resolution, skipped_funding_unavailable, failed_steps }
ExecutionPlanUpdated { aaa_id }
FundingBatchActivated { aaa_id, asset, amount }
FundingBatchPendingAccumulated { aaa_id, asset, added, pending_amount }
FundingBatchPromoted { aaa_id, asset, amount }
FundingSourcePolicyUpdated { aaa_id }
GlobalCircuitBreakerSet { paused: bool }
IdleStarvationDetected { consecutive_blocks: u32 }
LiquidityAdded { aaa_id, asset_a, asset_b, lp_minted }
LiquidityRemoved { aaa_id, lp_asset, amount_a, amount_b }
LiquidityDonated { aaa_id, asset_a, asset_b, amount, amount_a, amount_b }
ManualTriggerSet { aaa_id }
MintExecuted { aaa_id, asset, amount }
OnCloseExecutionPlanSummary { aaa_id, executed_steps, skipped_steps, failed_steps }
OnCloseExecutionPlanUpdated { aaa_id }
OnCloseStepFailed { aaa_id, step_index, kind: OnCloseStepFailureKind, error: DispatchError }
OnCloseStepSkipped { aaa_id, step_index, reason: StepSkippedReason }
ScheduleUpdated { aaa_id }
SplitTransferExecuted { aaa_id, asset, total, distributed, retained, legs: u32, effective_legs: u32 }
StakeExecuted { aaa_id, asset, amount }
StepFailed { aaa_id, cycle_nonce, step_index, error: DispatchError }
StepSkipped { aaa_id, cycle_nonce, step_index, reason: StepSkippedReason }
SwapExecuted { aaa_id, asset_in, asset_out, amount_in, amount_out }
SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 }
TransferExecuted { aaa_id, asset, amount, to }
UnstakeExecuted { aaa_id, asset, shares }
WakeupRescheduled { aaa_id, requested_block, scheduled_block }
WakeupScheduleDropped { aaa_id, requested_block }
```

### 11.2 Cycle Correlation

Indexer-facing correlation key is `(aaa_id, cycle_nonce)`.

Event ordering:

1. **Admitted cycle**: `CycleStarted` → zero or more step-level events (`StepSkipped` / `StepFailed` / task events) → `CycleSummary`; this ordering covers skip-only, all-failed-`ContinueNextStep`, and `AbortCycle` runs, while only the success predicate in Section 2.6 controls failure reset and auto-close eligibility.
2. **No admitted cycle**: weight rejection or a rolled-back scheduler close emits `CycleDeferred` without `CycleStarted`/`CycleSummary`; direct close-only execution emits only the close-tail sequence below.
3. **Terminal close with admitted close tail**: when closure follows an admitted successful normal cycle, that cycle MUST first end with `CycleSummary`; then zero or more close-tail task / `OnCloseStepSkipped` / `OnCloseStepFailed` events → `OnCloseExecutionPlanSummary` → `AaaClosed`. Direct close-only execution starts at the close-tail sequence and MUST NOT emit `CycleStarted`/`CycleSummary`.

Frontends SHOULD derive per-cycle step-status bitmask from `StepSkipped`/`StepFailed` events. `CycleSummary` is authoritative when present.

---

## 12. Type Reference

### 12.1 Core Types

```rust
enum AaaType {
    User,
    System,
}

enum Mutability {
    Mutable,
    Immutable,
}

enum CloseReason {
    AutoCloseNonceReached,
    BalanceExhausted,
    ConsecutiveFailures,
    CycleNonceExhausted,
    FeeBudgetExhausted,
    OwnerInitiated,
    WindowExpired,
}

enum DeferReason {
    InsufficientWeightBudget, CloseTransitionFailed,
}

enum StepSkippedReason {
    ConditionsNotMet,
    FundingUnavailable,
    ResolutionSkipped,
}

enum OnCloseStepFailureKind { EvaluationFee, ExecutionFee, Condition, Resolution, Adapter }

enum PauseReason {
    Manual,
    CycleNonceExhausted,
}

struct Schedule<AccountId, AssetId> {
    trigger: Trigger<AccountId, AssetId>,
    cooldown_blocks: u32,
}

enum AmountResolution<Balance> {
    Fixed(Balance),
    PercentageOfCurrent(Perbill),
    PercentageOfTrigger(Perbill),
    PercentageOfLastFunding(Perbill),
    AllBalance,
}

struct SplitLeg<AccountId> { to: AccountId, share: Perbill }

enum Condition<AssetId, Balance, BlockNumber> {
    BalanceAbove { asset: AssetId, threshold: Balance },
    BalanceBelow { asset: AssetId, threshold: Balance },
    BalanceEquals { asset: AssetId, threshold: Balance },
    BalanceNotEquals { asset: AssetId, threshold: Balance },
    BlockNumberAbove { threshold: BlockNumber },
    BlockNumberBelow { threshold: BlockNumber },
}

struct TaskParams { conditions: u32, split_legs: u32 }

enum Task<AccountId, AssetId, Balance> {
    Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
    SplitTransfer { asset: AssetId, amount: AmountResolution<Balance>, legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs> },
    SwapExactIn { asset_in: AssetId, asset_out: AssetId, amount_in: AmountResolution<Balance>, slippage_tolerance: Perbill },
    SwapExactOut { asset_in: AssetId, asset_out: AssetId, amount_out: AmountResolution<Balance>, slippage_tolerance: Perbill },
    Burn { asset: AssetId, amount: AmountResolution<Balance> },
    Mint { asset: AssetId, amount: AmountResolution<Balance> },
    AddLiquidity { asset_a: AssetId, asset_b: AssetId, amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance> },
    RemoveLiquidity { lp_asset: AssetId, amount: AmountResolution<Balance> },
    Stake { asset: AssetId, amount: AmountResolution<Balance> },
    DonateLiquidity { asset_a: AssetId, asset_b: AssetId, amount: AmountResolution<Balance>, max_ratio_error: Perbill },
    Unstake { asset: AssetId, shares: AmountResolution<Balance> },
}
```

### 12.2 Errors

```rust
enum Error {
    AaaAlreadyActive,
    AaaDormant,
    AaaIdOccupied,
    AaaIdOverflow,
    AaaNotFound,
    AaaPaused,
    ActiveAaaCapacityExceeded,
    ActiveAaaCountInvariant,
    ActorIdentityCapacityExceeded,
    ActorIdentityCountInvariant,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceIncrementZero,
    AutoCloseNonceOverflow,
    EmptyExecutionPlan,
    ExecutionDelayTooLong,
    ExecutionPlanExceedsOnIdleBudget,
    ExecutionPlanTooLong,
    FundingBatchOverflow,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidAutoCloseNonce,
    InvalidOwnerSlot,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    InvalidTriggerConfiguration,
    MintNotAllowedForUserAaa,
    NotGovernance,
    NotOwner,
    NotPaused,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    SnapshotUnavailable,
    SovereignAccountCollision,
    SystemAaaNotClosed,
}
```

`AaaIdOccupied` applies only to explicit-id System AAA reopen attempts where the requested `aaa_id` already has an active or dormant identity. `EmptyExecutionPlan` applies only to an active run plan; an empty close plan is valid. Active-only controls fail with `AaaDormant`, while activating an active/paused actor fails with `AaaAlreadyActive`.

Resolution-time non-terminal cases (`Skipped`, `FundingUnavailable`) are modeled as deterministic resolution outcomes, not `Error` variants.

---

## 13. Storage

> All collections MUST remain bounded by `Max*` constants. The reference `0.7.x` pre-launch line is a fresh-genesis baseline and does not support in-place upgrade from `0.6.x`; its storage version marks the current schema rather than migration history. After any downstream chain launches, its storage-layout changes MUST use versioned, idempotent, bounded `OnRuntimeUpgrade` migrations.

This section defines the stable storage surface. Actor cardinality/capacity, immutable-close lineage, and single-entry wakeup topology are behaviorally required contracts rather than replaceable caches. Derived readiness plus ingress overflow/dedup implementation stores remain architecture-owned unless promoted here explicitly.

- `NextAaaId` (`AaaId`): monotonic allocator; reopen never rewinds
- `ActorIdentity` (`Map<Blake2_128Concat(AaaId), Identity>`): class, owner, sovereign account, and creation lineage for active and dormant actors
- `ActorHot` (`Map<Blake2_128Concat(AaaId), HotState>`): active/paused lifecycle, scheduler pointers, attempt/cycle state, and compact admission facts; absent for dormant identities
- `ActorProgram` (`Map<Blake2_128Concat(AaaId), Program>`): schedule/window and bounded run/close plans; absent for dormant identities
- `ActorFunding` (`Map<Blake2_128Concat(AaaId), FundingState>`): funding policy, tracked assets, and batches; absent for dormant identities
- `ActorIdentityCount` (`u32`): transactionally maintained O(1) cardinality of `ActorIdentity`, bounded by `MaxActorIdentities`
- `ActiveAaaCount` (`u32`): transactionally maintained O(1) cardinality of active plus paused `ActorHot` entries; excludes dormant identities
- `ClosedSystemAaaIds` (`Map<Blake2_128Concat(AaaId), Mutability>`): System close tombstones governing reopen eligibility
- `QueueHead` / `QueueTail` (`QueueTicket = u64`): monotonic unconsumed interval and next append ticket
- `QueuePages` (`Map<Blake2_128Concat<QueuePageId>, BoundedVec<QueueEntry, QueuePageSize>>`): bounded physical pages for the logical FIFO; tickets derive from page and slot unless benchmarked production metadata explicitly stores them
- `WakeupIndex` (`Map<Blake2_128Concat(BlockNum), BoundedVec<AaaId, MaxWakeupBucketSize>>`) / `MinWakeupBlock` (`BlockNumber`): canonical temporal index and earliest unresolved bucket
- `ScheduledWakeupBlock` (`Map<Blake2_128Concat(AaaId), BlockNumber>`) / `WakeupRetryPending` (`Map<Blake2_128Concat(AaaId), bool>`): canonical single-entry wakeup pointer and durable saturation-retry marker per actor
- `WakeupScheduleDrops` (`u64`): counter of wakeups that could not be scheduled
- `ActorHot.queue_ticket` (`Option<QueueTicket>`): sole live queue membership and lazy-invalidation marker
- `AddressEventInbox` (`Map<Blake2_128Concat(AaaId), InboxState>`): `OnAddressEvent` pending latch
- `OwnerSlotMask` (`Map<Blake2_128Concat(AccountId), u8>`) / `SovereignIndex` (`Map<Blake2_128Concat(AccountId), AaaId>`): User-slot occupancy and active-or-dormant sovereign guard
- `ActiveActorLimit` (`u32`): governance-controlled operational cap constrained by hard and queue bounds; stored `0` resolves to the bounded runtime default for compatibility
- `SweepCursor` (`AaaId`): zombie sweep cursor
- `GlobalCircuitBreaker` (`bool`) / `IdleStarvationBlocks` (`u32`): scheduler halt and breaker-inactive zero-budget observability

---

## 14. Safety Invariants

Implementation is compliant iff all hold. Each invariant references its normative source:

1. AAA admits each housekeeping, queue, wakeup, close, and cycle operation against both remaining Weight dimensions, and runtime enforces `MinOnIdleReservePct` dispatchable headroom (Section 8.4; Section 9.2)
2. All loops and queues remain bounded by explicit `Max*` constants and no bounded operation executes unmetered (Section 1 item 2)
3. Slot allocation and active-or-dormant identity occupancy mutations are synchronous and race-safe (Section 1 item 8; Section 2.3)
4. Determinism holds for equal state/context, including deterministic timer jitter (Section 1 item 1; Section 7.1)
5. AAA exposes no probability or entropy input in `Timer`, configuration, errors, events, or embedding obligations (Section 7.1)
6. Adapters are deterministic and their runtime-derived upper bounds cover canonical iteration, quote search, storage proof, fee collection, and fixed rounding in both Weight dimensions (Section 3.2; Section 3.5)
7. No recurring rent accrual or touch-based rent debit exists (Section 4.2)
8. `create_user_aaa` charges non-refundable `AaaCreationFee` through one atomic runtime `FeeCollector` transaction (Section 1 item 5; Section 4.4)
9. First eligible execution is bounded by `MaxExecutionDelayBlocks` (Section 1 item 11; Section 7.4)
10. `reserved_fee_remaining` is transient, and `FeeNativeAsset` spend paths use `spendable_fee_native` (Section 2.1 native-asset terminology; Section 4.3)
11. Weight deferrals preserve `cycle_nonce`, `consecutive_failures`, and `last_cycle_block`; User fee insufficiency at cycle admission is terminal (Section 2.6 items 1, 3, 5, 6, 7; Section 4.1)
12. `manual_trigger_pending` clears on admitted cycle start, persists across deferrals/pause/resume, and always retains one bounded path to future eligibility (Section 7.3; Section 8.3)
13. `SplitTransfer` preserves amount conservation, rejects `sum(share_i) > 100%`, and skips ED-unsafe legs deterministically (Section 6.2)
14. Amount resolution never silently clamps and resolves through `Skipped` or `FundingUnavailable` when needed (Section 1 item 9; Section 5.3)
15. `OnAddressEvent` updates occur only through producer-owned adapter paths; each concrete event position is processed once, identical transfers remain distinct, expired ingress remains balance-only, typed funding authority gates mutation, checked overflow rolls back the originating producer, and only successful cycles promote pending batches (Section 2.5; Section 7.2)
16. Terminal close preserves sovereign balances and never performs automatic refund fan-out (Section 1 items 3 and 4; Section 2.4)
17. Close-tail execution uses the same task, condition, amount-resolution, adapter, and weight-upper-bound discipline as normal execution and MUST NOT recurse into another close (Section 2.4; Section 4.1; Section 5.5)
18. Explicit and automatic closes both use admitted close tails on the current line; automatic close paths defer until bounded `on_idle` budget can admit the tail, and fee depletion during close degrades into per-step observable failure without blocking final closure (Section 2.4; Section 4.5; Section 11.1)
19. Circuit breaker halts normal cycles and scheduler-owned automatic close tails without partial events while preserving bounded housekeeping plus explicit lifecycle and sweep cleanup paths (Section 8.3; Section 10.4)
20. Sweep remains bounded: `permissionless_sweep` is O(1) and `permissionless_sweep_many` is O(K ≤ MaxSweepPerBlock) (Section 8.5; Section 10.3)
21. `on_initialize` never dispatches AAA cycles, and starvation handling remains observability-only (Section 8.6 item 6; Section 9.1)
22. `TriggerSnapshot` is built once at cycle or admitted close-tail start, remains read-only, and is never persisted (Section 5.4)
23. `FundingUnavailable` is non-terminal, emits `StepSkipped`, and does not increment `consecutive_failures` (Section 2.5 item 8; Section 5.3)
24. Scheduler execution is strictly bounded and fully metered per touched page, inspected entry, actor probe, complete plan, wakeup, and ingress operation; `MaxExecutionsPerBlock` caps only successful execution attempts while RefTime and ProofSize remain the primary admission limits (Section 8.2; Section 8.4)
25. `ActiveActorLimit` satisfies `0 < limit <= min(MaxActiveActors, MaxQueueLength)`, transactionally maintained O(1) active count makes active creation/activation fail fast, and `ActorIdentityCount <= MaxActorIdentities` bounds dormant plus active identity state (Section 2.4; Section 10.1; Section 10.2)
26. Event-driven queueing uses one actor-local live ticket plus the block-start tail cutoff to enforce `executions(A, B) <= 1`; execution-created late enqueues persist beyond the cutoff (Section 8.1; Section 8.3)
27. Governance updates of `ActiveActorLimit` fail fast when `new_limit > MaxQueueLength`; the default/effective operational cap remains queue-bounded to avoid scheduler actor-loss under full activation (Section 10.2; Section 15)
28. Timer scheduling is hybrid and deterministic: cadence `<=1` uses queue continuation, delayed timers use the canonical `WakeupIndex`, and bounded jitter reduces synchronized wakeup bursts (Section 7.1 items 3, 4, 5; Section 8.1 item 3)
29. `IdleStarvationBlocks` increments only when a breaker-inactive post-housekeeping budget exhausts either Weight dimension, resets when both remain positive, and emits `IdleStarvationDetected` on threshold crossing only (Section 8.6 items 3, 4, 5; Section 9.2)
30. Dormant identities and custody-only accounts own no executable program, scheduler/readiness/funding state, recurring reads/writes, fee work, or cycle events; activate/deactivate transitions preserve identity, slots, and balances atomically (Section 2.4)

---

## 15. Runtime Constants

- `AaaCreationFee`: runtime-specific; non-refundable opening fee through `FeeCollector` into `FeeSink`
- `AaaPalletId`: `PalletId(*b"aaactor0")`; sovereign derivation id (Section 2.3)
- `ActiveActorLimit`: 1..`min(MaxActiveActors, MaxQueueLength)`; queue-bounded active-program cap
- `ConditionReadFee`: runtime-specific; reference default 0.0005 Native per condition
- `MaxActiveActors`: 10,000; hard cap for active plus paused programs
- `MaxActorIdentities`: runtime-specific hard cap for active plus dormant identities; MUST be at least `MaxActiveActors`
- `MaxQueueLength`: 1,024–16,384; maximum unconsumed physical occupancy including tombstones
- `QueuePageSize`: production-Wasm-selected bounded page size; page size is I/O granularity and MUST NOT cap per-block traversal
- `MaxWakeupBucketSize`: 1,024–16,384; one-block `WakeupIndex` bucket bound
- `MaxQueueInsertionsPerBlock`: 64–1,024; per-block enqueue cap before deferred wakeup
- `MaxSpilloverBlocks`: runtime-configurable bounded wakeup spillover horizon; reference default `8`
- `MaxWakeupsPerBlock`: 64–1,024; bounded overdue wakeup-drain throughput
- `MaxConditionsPerStep`: 4; condition bound per step
- `MaxConsecutiveFailures`: 10; terminal threshold; `MaxAutoCloseNonceHorizon`: reference default 10,000; current-relative future-target bound
- `MaxExecutionDelayBlocks`: 10 years in blocks; maximum first-execution deferral
- `MaxTimerJitterBlocks`: 32–128; deterministic timer jitter cap
- `MaxExecutionsPerBlock`: configurable hard safety ceiling for successful actor execution attempts; DEOS reference value 1,000, with actual execution determined by remaining RefTime and ProofSize
- `MaxQueueEntriesScannedPerBlock`: optional independent physical inspection ceiling only when benchmark evidence justifies it; MUST NOT alias `MaxExecutionsPerBlock`
- `MaxFundingTrackedAssets`: 3–10; assets tracked by `PercentageOfLastFunding` per AAA
- `MaxIdleStarvationBlocks`: 10–50; admitted-base execution-budget exhaustion threshold before starvation event
- `MaxK`: runtime-specific; adapter O(K) ceiling
- `MaxOwnerSlots`: 8; User AAA slot namespace (`u8` bitmask)
- `MaxSplitTransferLegs`: 8; split fan-out recipient bound
- `MaxSweepPerBlock`: 5; zombie sweep throughput
- `MaxSystemExecutionPlanSteps` / `MaxUserExecutionPlanSteps`: 10 / 3; actor-class bounds independently applied to both run and close plan vectors
- `MaxWhitelistSize`: 16; max source-filter whitelist length
- `MinOnIdleReservePct`: 50%; defines reference `GuaranteedOnIdleWeight`, covering the guaranteed scheduler envelope plus an admitted cycle and close tail in both Weight dimensions; optional compatibility-ingress drain, heavyweight wakeup-retry, and sweep-time terminal-close units consume available headroom and carry over durably when saturated. The other half is shared dispatch capacity, with no dedicated Operational reserve until a concrete critical call justifies one
- `MinUserBalance`: runtime-specific, `>= FeeNativeAsset` ED; pre-cycle user safety floor
- `MinWindowLength`: 100 blocks; minimum schedule window
- `StepBaseFee`: runtime-specific; reference default 0.002 Native per-step evaluation base fee
---

_End of specification._
