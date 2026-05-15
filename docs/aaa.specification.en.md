# AAA Specification

- **Component:** `pallet-aaa` (Account Abstraction Actors)
- **Version:** `0.4.0`
- **Date:** March 2026
- **Status:** Normative

> The key words **MUST**, **REQUIRED**, **SHALL**, **SHOULD**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this document are to be interpreted as described in RFC 2119.

---

## 0. Specification Maintenance Meta-Layer

This specification MUST stay at or below **1080 lines** (formatting-preserving count), add new normative content only with equal-or-greater removal of obsolete content, state rules as positive executable behavior unless a negative safety-critical constraint is required, keep normative facts single-sourced with references instead of duplication, preserve mandatory blank-line separation above and below numbered headings, and ensure every line carries normative meaning, traceability, or required implementation context.

---

## 1. Stability Contract

1. **Determinism:** Identical network state and block context MUST produce identical AAA behavior across all nodes.
2. **Bounded Work:** Every runtime path (`on_initialize`, `on_idle`, extrinsics) MUST execute in O(1) or O(K) with explicit, finite `Max*` constants.
3. **Destruction in Place:** On terminal conditions, actor state is removed atomically and balances remain on the sovereign account.
4. **No-Refund Contract:** The protocol MUST NOT perform automatic asset refund fan-out on close; balance recovery is owner-operated.
5. **Creation-Cost Internalization:** `create_user_aaa` MUST charge a non-refundable opening fee routed to `FeeSink` to cover long-tail maintenance of abandoned actors.
6. **Stateless Execution Plans:** Steps are independent and read state at execution time; mutable cross-step state is forbidden. Read-only per-run execution context (e.g., `reserved_fee_remaining`, `TriggerSnapshot`) is allowed.
7. **Predictable Failure:** Failures MUST resolve into one of: `Deferred`, `StepSkipped`, `StepFailed`, or `AaaClosed`.
8. **Synchronous Mutations:** Slot-allocation mutations MUST be persisted in the same extrinsic execution to prevent intra-block races.
9. **Saturating Arithmetic:** Intermediate fee/limit math MUST use saturating semantics. User-visible amount resolution MUST NOT silently overflow or underflow and MUST resolve deterministically (`Skipped` outcome or explicit failure).
10. **Execution-Context Correctness:** Rules MUST respect FRAME hook semantics (e.g., no reliance on current block hash during execution).
11. **Deferred-Horizon Cap:** Runtime MUST reject configurations that postpone first eligible execution beyond ten years.
12. **Spec-Impl Sync:** Runtime behavior MUST conform to this document in the same release window, and release CI MUST block shipment unless invariant-mapped tests for Section 14 pass.

---

## 2. Actor Model

### 2.1 Instance

- **Terminology:** An **Execution Plan** is the static bounded list of steps configured on the actor. An **Execution Run (Cycle)** is one admitted execution attempt of the current plan, identified by `(aaa_id, cycle_nonce)`. All external observability and indexer correlation MUST be run-centric. Execution plans, trigger filters, and actor-to-actor asset flows are part of the on-chain behavioral surface of AAA, but they operate inside the scheduler, fee, lifecycle, and safety contract of this runtime; within existing task, adapter, and safety limits, protocol workflow changes SHOULD prefer actor-graph reconfiguration over runtime rewrites.
- **Native-asset terminology:** `FeeNativeAsset` denotes the balance surface used for `AaaCreationFee`, per-step User fees, `MinUserBalance`, and fee reservation. Staking uses the generic `Stake { asset, amount }` task only; any native staking representation is a runtime-defined `AssetId` interpreted by `StakingOps`, not a separate AAA task.
- **Stable plan shape:** `execution_plan` MUST be non-empty. `on_close_execution_plan` MUST also be non-empty; creation extrinsics that omit it MUST inject the canonical `[Noop]` close plan. Actors that want no close-time side effects keep `[Noop]`; mutable actors MAY replace it through `update_on_close_execution_plan`.

```rust
struct AaaInstance<AccountId, BlockNumber, Balance> {
    aaa_id: u64,
    aaa_type: AaaType,
    sovereign_account: AccountId,
    owner: AccountId,
    owner_slot: u8,
    mutability: Mutability,
    is_paused: bool,
    pause_reason: Option<PauseReason>,
    auto_close_at_cycle_nonce: Option<u64>,
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow>,
    execution_plan: BoundedVec<Step, MaxSteps>,
    on_close_execution_plan: BoundedVec<Step, MaxSteps>,
    cycle_nonce: u64,
    last_cycle_block: BlockNumber,
    consecutive_failures: u32,
    manual_trigger_pending: bool,
    funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
    funding_snapshots: BoundedBTreeMap<AssetId, FundingSnapshot<Balance, BlockNumber>,
  MaxFundingTrackedAssets>,
    cycle_weight_upper: Weight,
    cycle_fee_upper: Balance,
    created_at: BlockNumber,
    updated_at: BlockNumber,
}

struct FundingSnapshot<Balance, BlockNumber> {
    amount: Balance,
    block: BlockNumber,
}
```

### 2.2 Types and Mutability

- **User AAA:** Subject to evaluation/execution fees and bounded by `MaxOwnerSlots` via user slot allocation.
- **System AAA:** Governance-created, exempt from User fee model, MAY be Mutable or Immutable, MUST NOT be limited by user slot count, and MUST keep `owner_slot = 0` as a storage/event compatibility sentinel.

Mutability rules:

- **Mutable:** control origin MAY pause/resume/update schedule/update execution plan/update on-close execution plan/set or increment auto-close target.
- **Control origin:** signed owner for both actor types; governance origin is additionally valid for System AAA only.
- **User Immutable:** owner mutation calls MUST fail with `ImmutableAaa`; a runtime MAY expose emergency governance override for User actors only.
- **System Immutable:** no runtime extrinsic, including governance/root, may mutate, pause/resume, manually trigger, close, or reopen the actor; only runtime upgrade may alter the invariant.
- `fund_aaa` MUST remain available for both mutability classes when the asset is tracked. `manual_trigger` MUST remain available for User AAA and System Mutable AAA unless another lifecycle gate rejects it; System Immutable `manual_trigger` MUST fail with `ImmutableAaa`.

### 2.3 Sovereign Derivation and Slot Allocation

1. **User AAA:** `seed = Blake2_256( SCALE(AaaPalletId, owner, owner_slot) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`.
2. **System AAA:** `seed = Blake2_256( SCALE(AaaPalletId, b"system", aaa_id) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`. Slotless: MUST NOT consume bits in `OwnerSlotMask`; stored/emitted `owner_slot` MUST remain `0` as a compatibility sentinel and MUST be interpreted together with `aaa_type`.
3. User slot bit MUST be cleared on User AAA destruction.
4. Recreating a User AAA with the same `(owner, owner_slot)` or reopening a closed System AAA with the same `aaa_id` MUST derive the same `sovereign_account`.
5. Collision check MUST guard only active AAA ownership of the same sovereign account; this case MUST fail with `SovereignAccountCollision`.
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
2. `reopen_system_aaa(aaa_id, mutability, ...)` is the only stable explicit-id System AAA creation path. It MAY reopen only a previously closed Mutable System AAA id and MUST fail with `SystemAaaNotClosed` otherwise.
3. `reopen_system_aaa` MUST fail with `AaaIdOccupied` if the requested `aaa_id` is already active.
4. System AAA creation/reopen MUST insert `SovereignIndex[sovereign_account] = aaa_id` atomically with `AaaInstances`; active occupancy of that sovereign account remains the only collision criterion.
5. `NextAaaId` MUST remain monotonic. Reopening a previously closed lower id MUST NOT rewind it.

### 2.4 Lifecycle

Terminal conditions:

- `fee_native_balance < MinUserBalance`: before cycle start → User `AaaClosed(BalanceExhausted)`
- `consecutive_failures >= MaxConsecutiveFailures`: after cycle failure → `AaaClosed(ConsecutiveFailures)`
- `current_block > schedule_window.end`: all touch points → `AaaClosed(WindowExpired)`
- `fee_native_balance < cycle_fee_upper`: scheduler admission → User `AaaClosed(FeeBudgetExhausted)`
- `cycle_nonce == u64::MAX`: after admission → User closes, System pauses with `CycleNonceExhausted`
- auto-close target reached after successful run → `AaaClosed(AutoCloseNonceReached)`

System AAA is exempt from `MinUserBalance` checks and MUST NOT auto-pause on `FundingUnavailable`; unresolved funding is modeled as `StepSkipped(FundingUnavailable)`. Runtime configuration MUST enforce `MinUserBalance >= ExistentialDeposit(FeeNativeAsset)`. System Mutable owner/governance `close_aaa` MUST stay available without funding/trigger preconditions to remove long-paused actors. System Immutable `close_aaa` MUST fail with `ImmutableAaa`.

`WindowExpired` MUST be evaluated at every lifecycle touch point (scheduler admission, sweep extrinsics, `manual_trigger`, `fund_aaa`, pause/resume, schedule/execution-plan update). If `current_block > schedule_window.end`, runtime closes before other mutations in that call. Schedule window eligibility is inclusive on `end`: `start <= current_block <= end`; closure starts only when `current_block > end`.

Lifecycle is a single state machine: `Created → Active → Ready → Admitted → Running → Completed/Deferred/Failed → TerminalPending → Closing → Closed`. Normal cycles are scheduler-owned runs of `execution_plan`, increment `cycle_nonce` at admission, and emit `CycleStarted`/`CycleSummary`. Close tails are terminal runs of `on_close_execution_plan`; they are not normal cycles, MUST NOT increment `cycle_nonce`, and emit close-tail events followed by `AaaClosed`. Lifecycle touch extrinsics MAY detect terminal state and enter the close path, but MUST NOT present that path as a normal cycle.

Close precedence is checkpoint-scoped and deterministic: `WindowExpired` dominates all external/admission touch points; for unpaused User AAA admission, `BalanceExhausted` dominates `FeeBudgetExhausted`; nonce exhaustion is the only after-admission / before-step transition; after an admitted cycle, `ConsecutiveFailures` is the only post-failure terminal close and `AutoCloseNonceReached` is the only post-success terminal close.

Before terminal state removal, runtime MUST enter close-tail execution for explicit and automatic close paths. The close tail uses the same task, condition, amount-resolution, error-policy, adapter, and weight-upper-bound semantics as the main plan. User close-tail admission derives `close_cycle_weight_upper`/`close_cycle_fee_upper`, reserves `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`, and builds a fresh `TriggerSnapshot`; System AAA uses the same execution semantics with zero User fee charging. Close-tail execution MUST NOT recurse into another close. If fee-native balance depletes or fee routing fails during admitted close execution, affected close steps MUST fail/skip observably while final closure still completes; whole-tail skip is not part of the current contract.

Close-tail contract matrix:

- Creation: omitted close plan injects canonical `[Noop]`; mutable actors MAY replace it later
- Explicit close: control origin enters close tail inline before deletion, independent of mutability
- Automatic close: scheduler admits close tail only when bounded budget fits; otherwise closure defers
- Sweep close: sweep is a lifecycle touchpoint; it may enter terminal close but MUST NOT admit a normal cycle
- Fee depletion: per-step close failure/skip is observable and MUST NOT block later steps or final closure
- Nonce: close tail MUST NOT increment `cycle_nonce` or emit `CycleStarted`/`CycleSummary`
- Deletion: remove actor/readiness/index state, clear User slot, preserve sovereign balances, emit `AaaClosed`
- Recovery: User reattaches with `create_user_aaa_at_slot`; System reopens with `reopen_system_aaa`

Create/close transitions MUST synchronize `AaaInstances` and `SovereignIndex`; queue/readiness entries MUST be removed, or deterministic stale queue entries MUST be ignored at pop. Direct post-destruction balance-rescue extrinsics remain out of the stable contract.

### 2.5 Funding Snapshots

The per-asset snapshot map `funding_snapshots` is the canonical baseline for `PercentageOfLastFunding` resolution (Section 5.3). The runtime maintains a `funding_tracked_assets` set to optimize execution.

Required behavior:

1. **Execution-Plan Scanning:** On creation, `update_execution_plan(aaa_id, execution_plan)`, or `update_on_close_execution_plan(aaa_id, on_close_execution_plan)`, runtime MUST scan BOTH execution plans (`execution_plan` + `on_close_execution_plan`) and populate `funding_tracked_assets` with every `AssetId` involved in a step that uses `PercentageOfLastFunding`. Any execution-plan update MUST fully recompute tracked assets and prune `funding_snapshots` entries for assets no longer tracked.
2. **Snapshot Update via `fund_aaa`:** `fund_aaa` is permissionless for any signed caller, transfers `amount > 0` from caller to the actor sovereign account, and MUST update `funding_snapshots[asset]` for BOTH User and System AAA only when `asset` is in `funding_tracked_assets`; untracked assets MUST fail with `SnapshotUnavailable`.
3. **Snapshot Update via `notify_address_event`:** Every `notify_address_event(aaa_id, asset, amount, source)` with `amount > 0` MUST update `funding_snapshots[asset]`, BUT ONLY if the actor is System AAA AND `asset` is in `funding_tracked_assets`.
4. Snapshot update MUST be independent of funding caller identity after the tracked-asset gate passes.
5. Funding events that update tracked snapshots MUST remain valid regardless of pause state; they MUST NOT imply automatic pause/resume transitions.
6. `FundingUnavailable` is a deterministic non-terminal execution outcome for both User and System AAA; it covers missing/zero tracked snapshots and tracked-balance overspend, while untracked assets remain `SnapshotUnavailable` and stale tracked snapshots remain valid until overwritten.
7. `cycle_weight_upper` and `cycle_fee_upper` are run-plan cache fields that MUST be recomputed on create/update execution plan and MUST only affect admission/preflight efficiency, not functional execution semantics. Close-tail upper bounds (`close_cycle_weight_upper`, `close_cycle_fee_upper`) MUST also remain deterministically derivable from `on_close_execution_plan` on create/update, whether cached or recomputed, and MUST NOT alter functional task semantics.

### 2.6 Failure Tracking

1. `consecutive_failures` increments only on cycle abort (`AbortCycle`); if `MaxConsecutiveFailures > 0`, the terminal cutoff is inclusive (`>=`).
2. `consecutive_failures` resets on successful cycle completion.
3. Deferrals MUST NOT increment `consecutive_failures`.
4. `update_execution_plan` (Mutable) MUST reset `consecutive_failures`.
5. `cycle_nonce` increments exactly once per admitted cycle start.
6. Deferred cycles MUST NOT increment nonce.
7. `last_cycle_block` MUST be updated to `current_block` exactly on admitted cycle start (`CycleStarted`), not on completion and not on deferral.
8. On nonce exhaustion: User AAA MUST close with `CycleNonceExhausted`; System AAA MUST pause with `PauseReason::CycleNonceExhausted`.

---

## 3. Adapters

All operations MUST go through typed adapters.

### 3.1 AssetOps

```rust
trait AssetOps<AccountId, AssetId, Balance> {
    fn transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn balance(who: &AccountId, asset: AssetId) -> Balance;
    fn minimum_balance(asset: AssetId) -> Balance;
    fn can_deposit(who: &AccountId, asset: AssetId, amount: Balance) -> bool;
    fn total_issuance(asset: AssetId) -> Balance;
}
```

**Balance semantics:** `balance()` MUST return the adapter-visible immediately transferable balance for the asset before any AAA-local reservation is applied. For `FeeNativeAsset` this is runtime policy (typically `free_balance` after adapter-level locks/reserves/holds); for assets without hold semantics it may equal total balance. AAA then derives `spendable_fee_native` by subtracting transient `reserved_fee_remaining` from `FeeNativeAsset` `balance()` only; non-`FeeNativeAsset` balances are passed through unchanged for spendability checks.

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
4. `SwapExactIn` receives `slippage_tolerance: Perbill` — the adapter computes `min_out` internally (e.g. `min_out = (1 - tolerance) × quote`). The pallet never touches pricing logic
5. `SwapExactOut` receives `slippage_tolerance: Perbill` and MUST deterministically resolve required input before swap execution
6. Slippage/routing logic remains inside the DEX adapter; AAA handles only `DispatchError` via `on_error`.

### 3.3 StakingOps

```rust
trait StakingOps<AccountId, AssetId, Balance> {
    fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), DispatchError>;
}
```

AAA MUST NOT encode runtime-specific staking topology such as collator choice, nomination custody, receipt naming, or native liquid-staking mechanics in the task enum. Runtime adapters MAY route `Stake { asset, amount }` for a native-asset `AssetId` into native staking, liquid staking, or another chain-local staking primitive, but those semantics remain adapter policy outside the AAA pallet contract.

### 3.4 Task Weight Contract

Runtime MUST expose deterministic worst-case bounds:

`fn weight_upper_bound(task: Task, params: TaskParams) -> Weight`

Requirements:

- State-independent for fixed params.
- Bounded by configured `Max*`.
- Always `>=` actual execution.
- Full-cycle admission uses sum of step upper bounds.
- Task-level `weight_upper_bound` MUST include worst-case event emission cost for events produced by successful task execution.
- Runtime admission accounting MUST include deterministic step/cycle overhead for non-task events (`CycleStarted`, `StepSkipped`, `StepFailed`, `CycleSummary`, and lifecycle events emitted on terminal transitions).

Runtime SHOULD classify tasks into coarse weight buckets to reduce maintenance fragility:

| Bucket          | Tasks                                                |
| --------------- | ---------------------------------------------------- |
| `SimpleAssetOp` | `Transfer`, `Burn`, `Mint`                           |
| `DexSwap`       | `SwapExactIn` / `SwapExactOut`                       |
| `DexLiquidity`  | `AddLiquidity`, `RemoveLiquidity`, `DonateLiquidity` |
| `Fanout`        | `SplitTransfer` (parameterized by `legs`)            |
| `Noop`          | `Noop`                                               |

---

## 4. Economics

System AAA is exempt from User fee charging in this section. All collected User fees MUST be routed to `FeeSink`. In compliant runtimes, `FeeSink` routing MUST be total and deposit-capable for `FeeNativeAsset` transfers. `create_user_aaa` MUST fail if fee-sink routing fails. If a non-compliant runtime still exposes fee-sink transfer failure during cycle or close-tail charging, cycle-path failures MUST map deterministically to `StepFailed` and obey the configured `StepErrorPolicy`, while close-tail charging failures MUST remain observable through `OnCloseStepFailed` and MUST NOT block later close steps or final closure; this path is misconfiguration handling, not intended steady-state behavior.

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
  Execution fee is charged once a step becomes executable, even if dispatch later fails; steps resolved to `Skipped` or `FundingUnavailable` do not incur execution fee.

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

`create_user_aaa` MUST charge `AaaCreationFee` in `FeeNativeAsset` and route it to `FeeSink`; opening fee is non-refundable (never returned on `close_aaa`), and creation MUST fail if sink transfer fails or payer cannot cover `AaaCreationFee` plus normal transaction fees (`InsufficientFee`); `create_system_aaa` is exempt.

### 4.5 Close-Tail Admission and Forecasting

`on_close_execution_plan` is the terminal tail of the same deterministic execution pipeline, not a fee-free cleanup exception.

1. Runtime MUST derive `close_cycle_weight_upper` and `close_cycle_fee_upper` from `on_close_execution_plan` using the same task bounds, fee formulas, and close-time observability overhead as normal cycle admission.
2. Runtime MUST treat explicit and automatic closes the same at close-tail entry: build a fresh `TriggerSnapshot`, initialize User close reservation as `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`, and reuse zero User fee reservation for System AAA.
3. Scheduler-driven automatic closes MUST reserve enough dispatch budget early enough to admit the close tail; if bounded `on_idle` budget cannot fit that tail yet, runtime MUST defer closure rather than converting the current line back into whole-tail skip semantics.
4. If fee-native balance becomes insufficient during admitted close execution, affected close steps MUST fail/skip observably and the runtime MUST continue deterministic closure without retry loops or terminal-pending state.
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

1. `PercentageOfCurrent` uses balance at step execution time.
2. `PercentageOfTrigger` uses cycle-start snapshot (Section 5.4).
3. `PercentageOfLastFunding` uses the amount captured in `funding_snapshots` for the asset being spent (Section 2.5).
4. `AllBalance` resolves to `spendable_current - minimum_balance(asset)` under `PreserveSpend` policy. Under `ExpendableSpend` and `Mint` policies, `AllBalance` resolves to full `spendable_current`.
5. Resolution outcomes are deterministic: `Resolved(amount)`, `Skipped` (e.g. tiny percentage rounds to zero), or `FundingUnavailable`.

Resolution policies — runtime MUST apply one per task:

- `PreserveSpend`: `Transfer`, `SplitTransfer`, `SwapExactIn`, `AddLiquidity`, `RemoveLiquidity`; subtract ED; require spendable source
- `Mint`: `Mint`, `SwapExactOut`; no ED subtraction; no spendability requirement
- `ExpendableSpend`: `Burn`; full spendable balance; source check MAY delegate to adapter

For `SwapExactOut`, `Mint` policy semantics apply only to resolving target output amount. The DEX adapter MUST still enforce actual input sufficiency and fail atomically if required input cannot be paid from actor balances.

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

1. At admitted cycle start, after fee reservation (Section 4.3), or at admitted close-tail entry after close-tail fee reservation, runtime MUST build a transient `TriggerSnapshot: Map<AssetId, Balance>`.
2. Scan execution-plan steps for all `PercentageOfTrigger` references and collect the unique `AssetId` set.
3. For each asset: `FeeNativeAsset` → `snapshot[FeeNativeAsset] = spendable_fee_native` (per Section 4.3); other → `snapshot[asset] = AssetOps::balance(sovereign, asset)`.
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
- `PauseActor` MUST NOT be part of stable `StepErrorPolicy`.

Atomicity:

- **Task-level:** atomic.
- **Execution-plan-level:** non-atomic (previous successful steps persist).

Task atomicity rules:

1. Single-op tasks satisfy atomicity by delegating to one adapter call.
2. Multi-op tasks (e.g. `SplitTransfer`) MUST execute within a transactional boundary (runtime storage transactions or adapter-level commit/rollback).
3. If any sub-operation fails, the task MUST revert all prior sub-operations. Partial task effects MUST NOT persist.

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
- `Noop`: observation/padding step

`SwapExactIn` parameter contract:

```rust
SwapExactIn {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

`slippage_tolerance` is passed directly to `DexOps`; adapter computes `min_out = (1 - slippage_tolerance) × quote(asset_in, asset_out, amount_in)`. `Perbill::zero()` requires exact quote, `Perbill::one()` accepts any output. If quote is unavailable (no pool / zero liquidity), swap fails with `DispatchError` handled by `on_error`.

`SwapExactOut` parameter contract:

```rust
SwapExactOut {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

For `SwapExactOut`, adapter MUST resolve required input deterministically and enforce an implied `max_in = (1 + slippage_tolerance) × quote_required_in(asset_in, asset_out, amount_out)`.

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

### 7.1 Timer and Entropy

```rust
enum Trigger<AccountId, AssetId> {
    Timer { every_blocks: u32, probability: Option<Perbill> },
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
2. First admitted cycle (`cycle_nonce == 0`) MUST NOT be blocked by cooldown.
3. Readiness MUST fail when `current_block - last_cycle_block < cooldown_blocks`.
4. `last_cycle_block` MUST be updated at admitted cycle start (`CycleStarted`) and therefore cooldown is anchored to admission, not completion.
5. For `Timer`, effective inter-cycle gap is `max(every_blocks, cooldown_blocks)`.
6. `manual_trigger` MAY set `manual_trigger_pending`, but cooldown MUST still gate admission.

Timer rules:

1. `every_blocks` MUST satisfy `0 < every_blocks <= MaxExecutionDelayBlocks`; otherwise fail with `ExecutionDelayTooLong`
2. `probability: None` means deterministic cadence; `Some(p)` enables probabilistic gate
3. Cadence `every_blocks <= 1` MUST use queue self-continuation (`NextQueue`) and MUST NOT use `WakeupIndex`
4. Cadence `every_blocks > 1` MUST schedule through the deterministic time-ordered wakeup index (`WakeupIndex`)
5. Deterministic anti-storm jitter SHOULD be applied for delayed timers (`every_blocks > 1`):
   `jitter_window = min(every_blocks / 4, MaxTimerJitterBlocks)`
   `jitter = Blake2_256(aaa_id) % jitter_window` when `jitter_window > 0`, else `0`; horizon validation applies to `every_blocks`, not `every_blocks + jitter`
   `target_block = current_block + every_blocks + jitter`
6. If `RequireSecureEntropyForProbabilisticTasks=true`, schedules with `0 < p < 1` and any task other than `Noop` MUST require `EntropyProvider::is_secure_for_financial_probability()` or fail with `InsecureEntropyProvider` at admission time
7. Those same strict financial schedules MUST use `EntropyProvider::secure_entropy_for_financial_probability(subject)` at execution time and MUST NOT fall back to block hashes; if secure entropy is unavailable, the probability gate is a readiness miss and the cycle does not execute
8. For non-strict probability sampling, the entropy fallback chain is deterministic: external provider `(aaa_id, current_block)` → `parent_hash` → `block_hash(current_block - 1)`; `block_hash(current_block)` is forbidden in runtime dispatch
9. Probability sampling occurs only after cadence/cooldown readiness is met; a miss is a readiness miss, not a defer/error/event, leaves nonce/manual/failure state unchanged, and MUST re-arm delayed timers
10. Final sampled entropy MUST be domain-separated with resolved entropy hash, `aaa_id`, and `cycle_nonce`; off-chain nondeterministic entropy is forbidden

### 7.2 OnAddressEvent

```rust
struct InboxState<BlockNumber> {
    is_pending: bool,
    generation: u64,
    last_event_block: BlockNumber,
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
```

Inbox model:

1. `AddressEventInbox` is per-AAA pending-latch (not an event queue); multiple matched events coalesce into one pending signal.
2. A matched inbound balance-increase event is semantically a trigger-message: it sets `is_pending = true`, increments `generation`, and updates `last_event_block`.
3. Coalescing is signal-level: one admitted cycle may consume balances accumulated from multiple matched events since the previous inbox consumption.

Rules:

- `SourceFilter::Whitelist` and `AssetFilter::Whitelist` MUST be non-empty and bounded by `MaxWhitelistSize`.
- Events without a concrete source account identifier MUST match only `SourceFilter::Any`.
- Scheduler readiness for this trigger MUST be `true` iff `is_pending == true`.
- When a cycle starts for an actor with `OnAddressEvent`, the inbox entry MUST be consumed atomically.
- If a new matched event arrives after consumption, the actor MUST become ready again on subsequent scheduler passes.

Ingress contract:

1. Runtime ingress to `OnAddressEvent` MUST go through a runtime-configured adapter interface (`AddressEventIngress` or equivalent) that ultimately invokes `notify_address_event*`.
2. Ingress strategy SHOULD be submit-first: producer paths with explicit hook points (AAA asset ops, TMC/router routing paths, XCM transactor paths) SHOULD submit directly through the adapter at successful transfer/mint completion.
3. Scanner ingestion MAY be used as fallback for non-hookable producer paths (e.g. generic `pallet-assets` direct transfer/mint extrinsics).
4. Producers and scanner paths MUST NOT mutate `AddressEventInbox` or `funding_snapshots` directly.
5. Source and asset filters MUST be evaluated in the same state transition as the inbox update.
6. Source invariant: when a concrete sender is available, ingress MUST preserve it exactly; `source = None` is valid only for inherently source-less paths.
7. Dedup invariant: runtime MUST apply deterministic same-block dedup across submit + scanner paths before effective inbox/snapshot side effects.
8. Funding snapshot behavior for `notify_address_event` is normative (Section 2.5) and MUST remain independent from trigger-filter matching.
9. Boundedness invariant: ingress implementation MUST enforce explicit scan/admission caps and MAY use bounded carry-over queueing for over-cap recognized events, drained before scanning new events in subsequent blocks.
10. Weight for inbox updates is paid by the originating transfer/mint path (submit-first) or by bounded scanner/queue processing in `on_idle` (fallback).

### 7.3 Manual Trigger

`manual_trigger` bypasses schedule timing only. It MUST NOT bypass admission or fee checks.

1. Calling `manual_trigger` on an eligible unpaused actor MUST set `manual_trigger_pending = true` and perform a bounded enqueue/schedule request; calling it on a paused actor MUST fail with `AaaPaused`; calling it on System Immutable AAA MUST fail with `ImmutableAaa`.
2. `manual_trigger_pending` MUST be cleared exactly when a cycle is admitted and `CycleStarted` is emitted.
3. Deferrals MUST NOT clear `manual_trigger_pending`.
4. If the actor closes before admission, the flag is removed with actor state deletion.
5. If the actor is paused after the flag is set, `manual_trigger_pending` MUST persist across `pause_aaa` / `resume_aaa` transitions.

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

The AAA runtime is a **deterministic event-driven actor runtime**. Actors are never polled globally; they are woken by explicit events (Timer, AddressEvent, Manual), asset ingress can function as a trigger-message, and larger protocol workflows may emerge from composed actor graphs, but all of that flows through the same two-layer scheduler and bounded admission model: an active run queue for execution-ready actors plus a temporal wakeup layer for future eligibility.

### 8.1 Architecture: Two-Layer Scheduler

1. **Active Run Queue (`CurrentQueue` + `NextQueue`):** Each `on_idle` pass seeds one bounded execution queue from `CurrentQueue` plus staged `NextQueue`; deferred carry-over is persisted back into queue storage at block end.
2. **Queue Continuation:** Cadence `every_blocks <= 1` re-admits actors via run-queue continuation (`NextQueue`) instead of timer indexing.
3. **Temporal Wakeup Layer (`WakeupIndex` + `MinWakeupBlock`):** Governs deferred eligibility and overdue wakeup admission for timer-delayed actors (`every_blocks > 1`) through canonical block-bucketed wakeup storage bounded by `MaxWakeupBucketSize`, plus an actor-keyed live wakeup pointer, with closed/missing actors dropped lazily when their due bucket is drained.
4. **Dedup Epochs:** `QueueEpoch` increments each block. `ActorQueueEpoch` tracks the last block an actor was queued, preventing multi-enqueue amplification within the same block.

### 8.2 Execution Flow

Each block MUST seed the active queue from `CurrentQueue + NextQueue`, drain overdue timers, ingest address events, pop actors in deterministic FIFO order up to `MaxExecutionsPerBlock`, then persist deferred/leftover actors into next-block queue storage and increment `QueueEpoch`.

### 8.3 Scheduler Liveness Matrix

- Queue carry-over: deferred or leftover actors persist in deterministic FIFO order and MUST be revalidated at pop
- Timer due: delayed wakeup moves to active queue; actor wakeup pointer clears when drained
- Timer probability miss: no cycle/event/failure; delayed timers MUST re-arm, every-block timers use continuation
- AddressEvent matched: set/keep inbox latch; enqueue best effort; overflow MUST NOT clear the latch
- Manual trigger: set flag and enqueue/schedule; deferral preserves flag until admitted cycle start
- Queue/wakeup full: spill deterministically; if no bucket fits, emit drop and retain source latch when any
- Paused actor: queued/wakeup entries MAY remain; pop skips without clearing manual or inbox latches
- Closed/missing actor: stale queue/wakeup entries are ignored; close removes canonical readiness/pointers
- Window expired at touch/pop: enter terminal close before normal mutation or execution
- Breaker active: bounded housekeeping may continue; normal cycles halt; sweep-time terminal close remains allowed

### 8.4 Enqueue Deduplication, Budget, and Fairness

1. An actor MUST NOT appear multiple times in the queue during one block, and delayed timers MUST keep at most one live future wakeup per actor; `ActorQueueEpoch` enforces block-local dedup.
2. The scheduler MUST enforce `MaxQueueInsertionsPerBlock`; overflow probes `requested_block..requested_block + MaxSpilloverBlocks`, then emits `WakeupScheduleDropped` and increments `WakeupScheduleDrops` if no bucket fits.
3. Runtime MUST reserve at least `MinOnIdleReservePct` for `on_idle`; cycles use the residual budget after bounded housekeeping.
4. If weight or `MaxExecutionsPerBlock` is reached, remaining actors carry forward through bounded next-block queue storage; high-density fairness target is nonce spread `<= 3` and starvation-free.

### 8.5 Sweep

1. `permissionless_sweep` and `permissionless_sweep_many` are lifecycle touchpoints only: they evaluate terminal liveness immediately and MUST NOT enqueue, admit, or execute normal cycles.
2. Breaker state MUST NOT block sweep-time liveness evaluation or terminal closure; if an actor remains alive, the call returns without queue mutation.
3. `SweepCursor` iteration and batch accounting MUST tolerate missing/closed `AaaId` entries and continue without aborting traversal.

### 8.6 Starvation Safeguard

Because actors are never globally polled, the protocol relies on the Bounded Double-Buffer plus explicit starvation telemetry to guarantee forward progress:

1. `MinWakeupBlock` MUST continuously advance when `WakeupIndex` entries are drained, including sparse-gap recovery after long halts or cursor gaps.
2. Scheduled execution (timer or event) MUST roll over across saturated blocks through queue carry-over and wakeup spillover; if bounded wakeup spillover is exhausted, runtime MUST surface the incident via `WakeupScheduleDropped` and `WakeupScheduleDrops`, and closed/missing actors MAY leave at most one stale future-bucket entry until lazy due-time drop.
3. `IdleStarvationBlocks` MUST increment only when the breaker is inactive and the remaining `on_idle` budget after bounded housekeeping is zero.
4. `IdleStarvationBlocks` MUST reset to zero as soon as positive post-housekeeping execution budget exists, including blocks where no actor is ready.
5. `IdleStarvationDetected` MUST emit exactly once on threshold crossing and MUST NOT repeat on every subsequent starved block.
6. Starvation telemetry is observability-only; it MUST NOT trigger emergency cycle execution or any alternate scheduler path.

---

## 9. Runtime Hooks

### 9.1 `on_initialize`

- MUST remain bounded and deterministic.
- MUST NOT dispatch AAA cycles.
- MAY do minimal bookkeeping.

### 9.2 `on_idle`

- MUST perform bounded housekeeping first (address-event ingress + zombie sweep).
- With breaker inactive: execute admitted cycles using the full remaining `on_idle` weight after housekeeping.
- With breaker active: skip cycle execution and run housekeeping only.
- MAY perform bounded lazy readiness/inbox transitions.
- MUST run the `IdleStarvationBlocks` state machine from Section 8.6 after bounded housekeeping determines the remaining execution budget.
- MUST NOT contain unbounded loops.

---

## 10. Extrinsics

### 10.1 Owner / Control Extrinsics

- `create_user_aaa(mutability, schedule, schedule_window, execution_plan)`: create actor, lowest free owner slot, default `[Noop]` close plan
- `create_user_aaa_at_slot(owner_slot, mutability, schedule, schedule_window, execution_plan)`: exact recovery slot, default `[Noop]` close plan
- `pause_aaa(aaa_id)`: pause actor (Mutable only)
- `resume_aaa(aaa_id)`: resume actor (Mutable only)
- `manual_trigger(aaa_id)`: set manual trigger flag
- `fund_aaa(aaa_id, asset, amount)`: deposit tracked asset
- `close_aaa(aaa_id)`: owner-initiated close, destruction in place
- `update_schedule(aaa_id, schedule, schedule_window)`: update schedule/window (Mutable only)
- `update_execution_plan(aaa_id, execution_plan)`: replace run plan and reset `consecutive_failures` (Mutable)
- `set_auto_close_at_cycle_nonce(aaa_id, target)`: set/clear bounded cycle lease target
- `increment_auto_close_nonce(aaa_id, by)`: extend cycle lease target (`by > 0`, checked-add, bounded horizon)
- `update_on_close_execution_plan(aaa_id, on_close_execution_plan)`: replace close-time plan (Mutable only)

`execution_plan` is the normative term for the run-step vector.
`pause_aaa`, `resume_aaa`, `manual_trigger`, `close_aaa`, `update_schedule`, `update_execution_plan`, `set_auto_close_at_cycle_nonce`, `increment_auto_close_nonce`, and `update_on_close_execution_plan` share the same control gate: signed owner for both actor types, plus governance origin for System AAA only; governance MUST NOT control User AAA through this path. After origin passes, System Immutable actors MUST reject these control paths with `ImmutableAaa`.

`create_user_aaa` MUST pay normal transaction fees, charge `AaaCreationFee` to `FeeSink` (Section 4.4), and enforce the deferred-horizon cap (Section 7.4).

`create_user_aaa` and `create_system_aaa` MUST fail with `ActiveAaaCapacityExceeded` when active AAA instance count reaches `ActiveActorLimit`.

### 10.2 Governance Extrinsics

- `create_system_aaa(mutability, ...)`: create Mutable or Immutable System AAA, default `[Noop]` close plan
- `reopen_system_aaa(aaa_id, mutability, ...)`: reopen a closed Mutable System AAA at same `aaa_id`, default `[Noop]` close plan
- `set_global_circuit_breaker(paused: bool)`: global scheduler stop/resume
- `set_active_actor_limit(new_limit: u32)`: operational cap update (`0 < new_limit <= min(MaxActiveActors, MaxQueueLength)`)

`create_system_aaa(mutability, ...)` MUST allocate the fresh `aaa_id = NextAaaId`. `reopen_system_aaa(aaa_id, mutability, ...)` is the only stable explicit-id governance path for closed Mutable System AAA and MUST preserve deterministic sovereign re-derivation without rewinding `NextAaaId`; detailed id/occupancy rules are defined in Section 2.3.

### 10.3 Tooling Extrinsics

| Extrinsic                        | Description                                                    |
| -------------------------------- | -------------------------------------------------------------- |
| `permissionless_sweep(aaa_id)`   | Force lifecycle evaluation for one actor (REQUIRED)            |
| `permissionless_sweep_many(ids)` | Bounded batch lifecycle evaluation (`len <= MaxSweepPerBlock`) |

### 10.4 Circuit Breaker

When breaker is active:

1. Scheduler MUST stop admitted cycle execution; bounded housekeeping and queue/inbox/wakeup bookkeeping MAY continue.
2. Creation extrinsics MUST fail with `GlobalCircuitBreakerActive`.
3. `fund_aaa`, `manual_trigger`, `close_aaa`, `permissionless_sweep`, and `permissionless_sweep_many` MUST remain operational; queued work executes only after breaker clears.

---

## 11. Observability

### 11.1 Events

```rust
AaaClosed { aaa_id, reason: CloseReason }
AaaCreated { aaa_id, owner, owner_slot, aaa_type, mutability, sovereign_account }
AaaFunded { aaa_id, asset, amount }
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
GlobalCircuitBreakerSet { paused: bool }
IdleStarvationDetected { consecutive_blocks: u32 }
LiquidityAdded { aaa_id, asset_a, asset_b, lp_minted }
LiquidityRemoved { aaa_id, lp_asset, amount_a, amount_b }
LiquidityDonated { aaa_id, asset_a, asset_b, amount, amount_a, amount_b }
ManualTriggerSet { aaa_id }
MintExecuted { aaa_id, asset, amount }
OnCloseExecutionPlanSummary { aaa_id, executed_steps, skipped_steps, failed_steps }
OnCloseExecutionPlanUpdated { aaa_id }
OnCloseStepFailed { aaa_id, step_index, error: DispatchError }
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

1. **Regular completion:** `CycleStarted` → step-level events (`StepSkipped` / `StepFailed` / task events) → `CycleSummary`.
2. **Funding depletion:** `CycleStarted` → `StepSkipped { reason: FundingUnavailable }` → `CycleSummary`.
3. **Terminal close with admitted close tail:** zero or more close-tail task events / `OnCloseStepFailed` events → `OnCloseExecutionPlanSummary` → `AaaClosed`.

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
    InsufficientWeightBudget,
}

enum StepSkippedReason {
    ConditionsNotMet,
    FundingUnavailable,
    ResolutionSkipped,
}

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
    Noop,
}
```

### 12.2 Errors

```rust
enum Error {
    AaaIdOccupied,
    AaaIdOverflow,
    AaaNotFound,
    AaaPaused,
    ActiveAaaCapacityExceeded,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceIncrementZero,
    AutoCloseNonceOverflow,
    EmptyExecutionPlan,
    ExecutionDelayTooLong,
    ExecutionPlanTooLong,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
    InsecureEntropyProvider,
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

`AaaIdOccupied` applies only to explicit-id System AAA reopen attempts where the requested `aaa_id` is already active. `EmptyExecutionPlan` applies to both run and close execution plans in the current stable contract.

Resolution-time non-terminal cases (`Skipped`, `FundingUnavailable`) are modeled as deterministic resolution outcomes, not `Error` variants.

---

## 13. Storage

> All collections MUST remain bounded by `Max*` constants. Storage-layout changes MUST use versioned, idempotent, bounded `OnRuntimeUpgrade` migrations.

This section defines the stable storage surface. `WakeupIndex` and `MinWakeupBlock` are part of the canonical temporal wakeup contract in the current stable line, not merely reference-runtime implementation notes. Runtime support stores such as `AaaReadiness`, `ActiveActorLimit`, ingress overflow/dedup state, and `LastIngressIngestBlock` remain implementation-documented in architecture docs unless promoted here explicitly.

- `NextAaaId` (`AaaId`): monotonic allocator; reopen never rewinds
- `AaaInstances` (`Map<Blake2_128Concat(AaaId), AaaInstance>`): full actor state
- `CurrentQueue` (`BoundedVec<AaaId, MaxQueueLength>`): block-local run queue
- `NextQueue` (`BoundedVec<AaaId, MaxQueueLength>`): staged queue merged on next `on_idle`
- `WakeupIndex` (`Map<Blake2_128(BlockNum), BoundedVec<AaaId, MaxWakeupBucketSize>>`): canonical wakeup index
- `MinWakeupBlock` (`BlockNumber`): earliest unresolved wakeup block
- `WakeupScheduleDrops` (`u64`): counter of wakeups that could not be scheduled
- `ActorQueueEpoch` (`Map<Blake2_128Concat(AaaId), u64>`): per-actor enqueue deduplication
- `QueueEpoch` (`u64`): global queue generation counter
- `AddressEventInbox` (`Map<Blake2_128Concat(AaaId), InboxState>`): `OnAddressEvent` pending latch
- `OwnerSlotMask` (`Map<Blake2_128Concat(AccountId), u8>`): User-slot occupancy bitmask
- `SovereignIndex` (`Map<Blake2_128Concat(AccountId), AaaId>`): active sovereign-account guard
- `SweepCursor` (`AaaId`): zombie sweep cursor
- `GlobalCircuitBreaker` (`bool`): pallet-wide scheduler halt
- `IdleStarvationBlocks` (`u32`): consecutive zero-budget blocks while breaker inactive

---

## 14. Safety Invariants

Implementation is compliant iff all hold. Each invariant references its normative source:

1. AAA uses the full remaining `on_idle` budget after bounded housekeeping, and runtime enforces `MinOnIdleReservePct` dispatchable headroom (Section 8.4 item 1; Section 9.2)
2. All loops and queues remain bounded by explicit `Max*` constants (Section 1 item 2)
3. Slot allocation and active-occupancy mutations are synchronous and race-safe (Section 1 item 8; Section 2.3)
4. Determinism holds for equal state/context, including entropy fallback order (Section 1 item 1; Section 7.1)
5. Current-block hash is never used as entropy in runtime execution (Section 1 item 10; Section 7.1 item 8)
6. Adapters are deterministic: canonical iteration and fixed rounding (Section 3.2)
7. No recurring rent accrual or touch-based rent debit exists (Section 4.2)
8. `create_user_aaa` charges non-refundable `AaaCreationFee` and routes it to `FeeSink` (Section 1 item 5; Section 4.4)
9. First eligible execution is bounded by `MaxExecutionDelayBlocks` (Section 1 item 11; Section 7.4)
10. `reserved_fee_remaining` is transient, and `FeeNativeAsset` spend paths use `spendable_fee_native` (Section 2.1 native-asset terminology; Section 4.3)
11. Weight deferrals preserve `cycle_nonce`, `consecutive_failures`, and `last_cycle_block`; User fee insufficiency at cycle admission is terminal (Section 2.6 items 1, 3, 5, 6, 7; Section 4.1)
12. `manual_trigger_pending` clears on admitted cycle start and persists across deferrals, pause, and resume (Section 7.3)
13. `SplitTransfer` preserves amount conservation, rejects `sum(share_i) > 100%`, and skips ED-unsafe legs deterministically (Section 6.2)
14. Amount resolution never silently clamps and resolves through `Skipped` or `FundingUnavailable` when needed (Section 1 item 9; Section 5.3)
15. `OnAddressEvent` updates occur only through the adapter path, with deterministic and bounded matching/dedup semantics (Section 7.2)
16. Terminal close preserves sovereign balances and never performs automatic refund fan-out (Section 1 items 3 and 4; Section 2.4)
17. Close-tail execution uses the same task, condition, amount-resolution, adapter, and weight-upper-bound discipline as normal execution and MUST NOT recurse into another close (Section 2.4; Section 4.1; Section 5.5)
18. Explicit and automatic closes both use admitted close tails on the current line; automatic close paths defer until bounded `on_idle` budget can admit the tail, and fee depletion during close degrades into per-step observable failure without blocking final closure (Section 2.4; Section 4.5; Section 11.1)
19. Circuit breaker halts scheduler execution while preserving bounded housekeeping and cleanup/tooling paths (Section 10.4)
20. Sweep remains bounded: `permissionless_sweep` is O(1) and `permissionless_sweep_many` is O(K ≤ MaxSweepPerBlock) (Section 8.5; Section 10.3)
21. `on_initialize` never dispatches AAA cycles, and starvation handling remains observability-only (Section 8.6 item 6; Section 9.1)
22. `TriggerSnapshot` is built once at cycle or admitted close-tail start, remains read-only, and is never persisted (Section 5.4)
23. `FundingUnavailable` is non-terminal, emits `StepSkipped`, and does not increment `consecutive_failures` (Section 2.5 item 6; Section 5.3)
24. Scheduler execution is strictly bounded by `MaxExecutionsPerBlock`, `MaxWakeupsPerBlock`, `MaxQueueInsertionsPerBlock`, and `MaxWakeupBucketSize`; wakeup-overflow incidents are observable via `WakeupScheduleDropped` / `WakeupScheduleDrops` (Section 8.3; Section 8.4)
25. `ActiveActorLimit` satisfies `0 < limit <= min(MaxActiveActors, MaxQueueLength)`, and creation fails fast at capacity (Section 10.1; Section 10.2)
26. Event-driven queueing enforces strict block-local `ActorQueueEpoch` deduplication to prevent amplification attacks (Section 8.1 item 4; Section 8.3)
27. Governance updates of `ActiveActorLimit` fail fast when `new_limit > MaxQueueLength`; the default/effective operational cap remains queue-bounded to avoid scheduler actor-loss under full activation (Section 10.2; Section 15)
28. Timer scheduling is hybrid and deterministic: cadence `<=1` uses queue continuation, delayed timers use the canonical `WakeupIndex`, and bounded jitter reduces synchronized wakeup bursts (Section 7.1 items 3, 4, 5; Section 8.1 item 3)
29. `IdleStarvationBlocks` increments only on breaker-inactive zero post-housekeeping budget, resets on positive budget, and emits `IdleStarvationDetected` on threshold crossing only (Section 8.6 items 3, 4, 5; Section 9.2)

---

## 15. Runtime Constants

- `AaaCreationFee`: runtime-specific; non-refundable opening fee to `FeeSink`
- `AaaPalletId`: `PalletId(*b"aaactor0")`; sovereign derivation id (Section 2.3)
- `ActiveActorLimit`: 1..`min(MaxActiveActors, MaxQueueLength)`; queue-bounded creation cap
- `ConditionReadFee`: 0.0005 Native; per-condition read fee
- `MaxActiveActors`: 10,000; hard cap for active AAA instances
- `MaxQueueLength`: 1,024–16,384; `CurrentQueue`/`NextQueue` bound
- `MaxWakeupBucketSize`: 1,024–16,384; one-block `WakeupIndex` bucket bound
- `MaxQueueInsertionsPerBlock`: 64–1,024; per-block enqueue cap before deferred wakeup
- `MaxSpilloverBlocks`: 8; wakeup spillover horizon in blocks
- `MaxWakeupsPerBlock`: 64–1,024; bounded overdue wakeup-drain throughput
- `MaxConditionsPerStep`: 4; condition bound per step
- `MaxConsecutiveFailures`: 10; terminal threshold
- `MaxExecutionDelayBlocks`: 10 years in blocks; maximum first-execution deferral
- `MaxTimerJitterBlocks`: 32–128; deterministic timer jitter cap
- `MaxExecutionsPerBlock`: 16–64; global per-block admitted execution cap
- `MaxFundingTrackedAssets`: 3–10; assets tracked by `PercentageOfLastFunding` per AAA
- `MaxIdleStarvationBlocks`: 10–50; zero-`on_idle` threshold before starvation event
- `MaxK`: runtime-specific; adapter O(K) ceiling
- `MaxOwnerSlots`: 8; User AAA slot namespace (`u8` bitmask)
- `MaxSplitTransferLegs`: 8; split fan-out recipient bound
- `MaxSweepPerBlock`: 5; zombie sweep throughput
- `MaxSystemExecutionPlanSteps` / `MaxUserExecutionPlanSteps`: 10 / 3; System/User step bounds
- `MaxWhitelistSize`: 16; max source-filter whitelist length
- `MinOnIdleReservePct`: 10%; block-weight headroom reserved for `on_idle` paths
- `MinUserBalance`: runtime-specific, `>= FeeNativeAsset` ED; pre-cycle user safety floor
- `MinWindowLength`: 100 blocks; minimum schedule window
- `StepBaseFee`: 0.001 Native; per-step evaluation base fee

---

_End of specification._
