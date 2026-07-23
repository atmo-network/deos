# AAA (Account Abstraction Actors): Runtime Architecture

> **On-Chain Namespace**
>
> - Pallet: `pallet-aaa`
> - PalletId: `aaactor0`
> - Pallet account (`PalletId(*b"aaactor0").into_account_truncating()`, SS58 prefix 42):
>   - hex: `0x6d6f646c61616163746f72300000000000000000000000000000000000000000`
>   - SS58: `5EYCAe5fiK3ZpinaPEDXwvtT6tFp5gBL16S5vyt4TYmgLaT1`
> - Sovereign derivation:
>   - User AAA: `blake2_256(PalletId, owner, owner_slot)`
>   - System AAA: `blake2_256(PalletId, "system", aaa_id)`; `ActorClass::System` stores no owner slot, while the legacy creation-event field remains at compatibility sentinel `0` and never consumes `OwnerSlotMask`

## Executive Summary

`pallet-aaa` is the execution platform for deterministic protocol behavior in DEOS. It provides a deterministic scheduler, bounded execution model, and adapter-driven task runtime for both user-owned and system-owned actors.

The current DEOS reference runtime reserves fifteen deterministic System addresses for the TMCTOL topology but enrolls only actual programs in the scheduler:

- Active actors: Burn Actor `0`, Fee Sink `1`, and BLDR Splitter `10`; all three use omnivorous `OnAddressEvent` intake rather than periodic polling.
- Dormant identities: Liquidity Actor `2`, TOL Buckets B/C/D `4..=6`, Treasuries B/C/D `7..=9`, BLDR Liquidity Actor `11`, BLDR Treasury `13`, and native staking LP provisioning actor `14`.
- Custody-only accounts outside generic actor storage: TOL Bucket A `3` and BLDR Bucket A `12`.

Dormant identities retain their deterministic sovereign address, owner, Mutable System class, and genesis provider but have no `ActorHot`, `ActorProgram`, readiness, inbox, queue, wakeup, funding, fee, or cycle state. Custody-only accounts receive only a runtime-declared genesis provider so assets can enter safely; they have no generic AAA identity or control surface.

The pallet itself is generic and runtime-agnostic. Chain-specific behavior is injected through `AssetOps`, `DexOps`, weight/fee conversion, bounds, and genesis actor specs. External host-runtime obligations are summarized in the [AAA External Runtime Embedding Guide](./aaa.embedding.en.md).

### Canonical Address Catalog (Current Runtime, Full System Map)

All addresses below are deterministic for `AaaPalletId = *b"aaactor0"` (`AccountId32`, SS58 prefix 42).
This catalog is the full current runtime deterministic System address map, including active actors, dormant identities, and custody-only accounts.

- aaa_id: `—`; pallet-aaa` (pallet account)
  - Hex: `0x6d6f646c61616163746f72300000000000000000000000000000000000000000`
  - SS58: `5EYCAe5fiK3ZpinaPEDXwvtT6tFp5gBL16S5vyt4TYmgLaT1`
- aaa_id: `0`; Burn Actor
  - Hex: `0xeba61f8494ba498cb84ce3b771bc3c193dbd82f9a999153a55c383349f6e512e`
  - SS58: `5HPgTa8GLrmzMDktPEWmuC82WtipKSibwd9C2pUQnESn4nAv`
- aaa_id: `1`; Fee Sink
  - Hex: `0xab373631522954b038699419fadc732893dff1230239bc30fbe17bf5fb12f084`
  - SS58: `5FwCSs6WuW2tTv7uQFRB1o4rjmPQsgE6PesjKUUbroxfzKKh`
- aaa_id: `2`; Liquidity Actor
  - Hex: `0xb136dc3f6dba4aac24a8c9f8be3c7b20e26b08422803b6999b7cd019c4ca50ab`
  - SS58: `5G54dUVans8Rvnn1qdTea3fQ28osh8T7ijaWbi3gygm9sa7C`
- aaa_id: `3`; TOL Bucket A (Anchor)
  - Hex: `0x6f9a5aa8cd9ba27b2e69f1bac1c521d2ffde543275ebd787da11dbd131c50d25`
  - SS58: `5Eb32Qkj9FpPMUXZMNreJzRESQRbYQWwiKXK4zf9VXifTEqX`
- aaa_id: `4`; TOL Bucket B (Building)
  - Hex: `0x03699bb4549d77d91390fc161867ccd3ef97d4f305f01757708905c84cb7d882`
  - SS58: `5C9BNb4AoxDngwC6nzu8SEtAEbtGHiKeBjzJwgUewA9qDNL3`
- aaa_id: `5`; TOL Bucket C (Capital)
  - Hex: `0x313e7fb07ed6681741b54c3d421f8c261027048e2a9b0668e1058654d369de29`
  - SS58: `5DBGmawvmUvHAg9e2A4bcwZm3NiGX5KE5sPCKepN36SMJvfX`
- aaa_id: `6`; TOL Bucket D (Dormant)
  - Hex: `0xd23baab9890a6990ff23e7ad7ab9d1ad34712d7add2344917d110e3cec5b9242`
  - SS58: `5GpMdwY6iMiA8LRUczsZH6p9WoxN4rX15U7FJWbeqTqTrPLX`
- aaa_id: `7`; Treasury B (Building)
  - Hex: `0xa027809984f38031e61246efe8ad1f28ddacd9870f6bed081560089c15f9b966`
  - SS58: `5FghFeZDxtGWmvASpM4etxnYtreW9yamSx1Pwh1aGYkny2uv`
- aaa_id: `8`; Treasury C (Capital)
  - Hex: `0xcae77c85e5665e0cbe994898429478d3facf4c29a9b7539902f95ad7b3b4bf9b`
  - SS58: `5GekJ6zNwu6ABqhpcagnxbPmP6UtJ1gUKdvJywZKugWkCLhe`
- aaa_id: `9`; Treasury D (Dormant)
  - Hex: `0xc81b0eb40aea260eb09b950cfbe2c43f9be1dc73bf62cf081c376cff4bdae0ca`
  - SS58: `5Gb5UKWyYyyttHG3GCsyEhN2Qtb92auewWLZzPaQCvp1RHaj`
- aaa_id: `10`; BLDR Splitter
  - Hex: `0x8a420d09aa8842c9075deefab7791be5e9f9471bc68baa8c926128cfc29b6962`
  - SS58: `5FBz5y9kWN7ArW1w5TZiCLbszGmG3FmCSx6njj9w7VEuiK8N`
- aaa_id: `11`; BLDR Liquidity Actor
  - Hex: `0x6324e98949d19dbe10162a939df82b28368bef743a14aa8ce0a3d9a02d567221`
  - SS58: `5EJhZc6rdqBKzZcJXfjeMwTaQvYsyTF9YJS39sWr1HEuEy17`
- aaa_id: `12`; BLDR Bucket A (Anchor)
  - Hex: `0xb31a379c50afe1ba1ad65f1afafaf51df1c40ed2b6c08e9faf1a1ac2caf026de`
  - SS58: `5G7YDX7r2L8q5Wn73dNyhp8cnbpP3sTGUcRW6Eos5Urrxax8`
- aaa_id: `13`; BLDR Treasury
  - Hex: `0x3a1bedf666c4852432a75dc0099fec586a02b813acb4457c9d4b150a03bdce45`
  - SS58: `5DNtvy5YymuvPBM6Wk8ADHs9ggLK2gjEZoaSoeM3aHLykNKG`
- aaa_id: `14`; Native staking LP provisioning actor
  - Hex: `0xbb27f4956462189d16c7f9e207222ce9691308c6a55bb0141f139ebe071394d2`
  - SS58: `5GJ6gSae5dZhxJm6EuD82gaxiLkvokMeLFMNmtuSz8htoidu`

---

## Architecture Overview

### Design Principles

1. `Deterministic scheduling`: queue-driven double-buffer execution with deterministic ordering and explicit per-block caps
2. `Execution safety`: explicit weight and fee admission gates before cycle start
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`FeeBudgetExhausted`, `BalanceExhausted`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `Hot-state decomposition`: ActorHot remains the Slice 4 target for avoiding cold-plan decode; the retired synchronized readiness mirror must not return

### Runtime Role in DEOS

AAA is the automation substrate inside DEOS. It executes domain logic via declarative execution plans while specialized pallets keep their domain mechanics. In the current reference line, those plans realize the TMCTOL standard:

- `pallet-axial-router`: routing and price-aware swaps
- `pallet-tmc`: minting curve mechanics
- `pallet-asset-conversion`: AMM liquidity mechanics
- `pallet-assets` / `pallet-balances`: token ledgers

AAA orchestrates actions against these pallets through adapter traits.

---

## Current TMCTOL System AAA Topology on DEOS

### Genesis actor matrix

`TmctolGenesisSystemAaas` currently classifies fifteen deterministic addresses:

| Lane | Role | aaa_id | Genesis lifecycle |
| --- | --- | ---: | --- |
| Core | Burn Actor | 0 | Active: `build_burn_execution_plan([], dust)` |
| Core | Fee Sink | 1 | Active: Phase 1 50/50 staking + LP donation |
| Core | Liquidity Actor | 2 | Dormant |
| TOL | Bucket A (Anchor) | 3 | Custody-only account |
| TOL | Bucket B (Building) | 4 | Dormant |
| TOL | Bucket C (Capital) | 5 | Dormant |
| TOL | Bucket D | 6 | Dormant |
| Treasury | Treasury B | 7 | Dormant |
| Treasury | Treasury C | 8 | Dormant |
| Treasury | Treasury D | 9 | Dormant |
| BLDR | BLDR Splitter | 10 | Active: `build_bldr_splitter_execution_plan` |
| BLDR | BLDR Liquidity Actor | 11 | Dormant |
| BLDR | BLDR Bucket A | 12 | Custody-only account |
| BLDR | BLDR Treasury | 13 | Dormant |
| Staking | Native staking LP provisioning actor | 14 | Dormant |

Active genesis actors use `SYSTEM_AAA_COOLDOWN_BLOCKS`, `AaaType::System`, `Mutability::Mutable`, and no schedule window. Dormant entries occupy `DormantAaaIdentities` and `SovereignIndex` but do not count toward `ActiveAaaCount`; custody-only accounts occupy neither store. `ActorIdentityCount` covers the thirteen active plus dormant identities. `NextAaaId` remains `15`, preserving the reserved address range.

### Canonical execution-plan families

The runtime keeps System AAA topology declarative. Governance evolves concrete execution plans through builder functions in `runtime/src/configs/aaa_config.rs`:

- `build_burn_execution_plan` / Burn Actor: foreign balances → Native swap → burn; extend after foreign asset registration
- `build_zap_execution_plan` / Liquidity Actor: add LP, foreign→native swap, split LP to buckets; activate after pool creation
- `build_bucket_lp_transfer_execution_plan` / Buckets B/C/D: transfer a bounded LP fraction into the paired Treasury as one admitted cycle
- `build_treasury_lp_unwind_execution_plan` / Treasuries B/C/D: remove all preservable balance of one configured system LP asset as a separate admitted cycle, leaving both underlying assets in Treasury custody; the task does not restrict who supplied that LP
- `build_bldr_splitter_execution_plan` / BLDR Splitter: split the minted BLDR share 50/50 to liquidity/treasury lanes; TMC routes NTVE collateral directly to the BLDR Liquidity Actor; active at genesis
- `build_bldr_zm_execution_plan` / BLDR Liquidity Actor: add NTVE/BLDR liquidity and transfer LP to BLDR Bucket A; activate after pool creation
- `build_treasury_b_buyback_execution_plan` / Treasury B: swap NTVE percentage into target and burn acquired balance; optional policy lane
- `build_native_staking_lp_farming_execution_plan` / Native staking LP provisioning actor: donate balanced `NTVE/stNTVE` without minting LP; activate after native pool + AMM creation

This split keeps AAA generic: the pallet owns bounded scheduling/execution, while the current DEOS reference runtime wires the TMCTOL standard's economic composition into concrete System actors. Reusable execution-plan examples should be read as task-language patterns; the actor catalog above is TMCTOL-specific topology, not a required shape for downstream `pallet-aaa` adopters.

### Governance activation flows

Current runtime operations reduce to four repeatable flows:

1. `Foreign asset + TOL lane`
   Register foreign asset -> create Native/foreign pool -> update Burn Actor -> update Liquidity Actor -> optionally activate a Bucket LP-transfer plan and the corresponding Treasury LP-removal plan. Pairing expresses reference topology and custody intent, not sender authorization: Treasury unwinds its configured LP balance regardless of depositor. The two cycles replace the rejected three-step same-actor unwind without bypassing `GuaranteedOnIdleWeight`.
2. `BLDR lane`
   Keep BLDR Splitter live at genesis -> create NTVE-BLDR pool -> activate BLDR Liquidity Actor -> optionally activate Treasury B buyback/burn policy.
3. `Native staking LP provisioning lane`
   Register native staking -> initialize `stNTVE` -> create and seed the `NTVE/stNTVE` AMM -> call `activate_native_staking_lp_farming`, which refuses activation until the receipt asset, staking pool, actor, and non-empty AMM are all live.
4. `Emergency controls`
   Pause single actors with `pause_aaa` when policy needs surgical intervention; use `set_global_circuit_breaker(true)` when cycle execution as a whole must stop while bookkeeping stays alive.

---

## Execution Model

### Actor Classes

| Class | Ownership | Mint task allowed | Typical usage |
| --- | --- | --- | --- |
| `User` | Signed owner + slot namespace | No | User automation |
| `System` | Governance origin | Yes | Protocol automation |

User recovery now has an explicit slot-targeted surface: the default `create_user_aaa` path allocates the lowest free slot, while `create_user_aaa_at_slot` recreates control for a chosen slot/sovereign deterministically.

Current owner-slot representation is intentionally compact and runtime-shaped:

- `OwnerSlotMask` is stored as a `u8` occupancy bitmask for the bounded user-slot namespace
- Bits above `MaxOwnerSlots` are masked away before allocation decisions
- The current representation is little-endian in the usual `(1 << n) - 1` sense
- Default allocation walks the lowest free bit first, while the exact-slot path fails if the requested bit is already occupied

### Current Actor-State Shape

The shipped runtime stores each active actor across three canonical values:

- `ActorHot`: identity/ownership, typed lifecycle, cycle nonce, canonical `first_eligible_at`, last admitted cycle block, failure counter, manual-trigger flag, live queue ticket, optional last User queue-mutation block, auto-close target, and cached cycle weight/fee bounds
- `ActorProgram`: trigger/cooldown schedule, optional execution window, run plan, and close plan
- `ActorFunding`: funding policy, tracked assets, bounded armed/pending batches, and canonical pending indication

`aaa_id` exists only as each storage-map key. Dormant identity carries no timestamp. Activation or a pre-first-cycle schedule update derives first eligibility from current block, window start, cadence, and actor-stable jitter. The typed lifecycle forbids contradictory pause state. Scheduler admission, cycle execution, control extrinsics, lifecycle transitions, liveness, wakeup, ingress, try-state, benchmarks, and tests compose explicit `ActorHot + ActorProgram` snapshots through pallet-private helpers or the public `aaa_instances` Rust query helper. Browser and operator clients read `ActorHot` and `ActorProgram` at the same finalized snapshot and require both for an active actor projection; the Automation widget reports this split provenance directly. The temporary Rust `AaaInstances` compatibility facade is deleted; no Rust type or metadata storage prefix carries that legacy name.

This is intentionally more concrete than the paired specification: the spec defines the required logical field groups, while this document records the current runtime-shaped storage realization.

### Execution-Plan Structure

Each actor stores a bounded `ExecutionPlan` of ordered `Step`s:

- `conditions: BoundedVec<Condition, MaxConditionsPerStep>`
- `task: Task`
- `on_error: StepErrorPolicy` (`AbortCycle` / `ContinueNextStep`)

Task set in implementation:

- `Transfer`
- `SplitTransfer`
- `SwapExactIn`
- `SwapExactOut`
- `AddLiquidity`
- `RemoveLiquidity`
- `Burn`
- `Mint` (System only)
- `Stake`
- `DonateLiquidity`
- `Unstake`

### Amount Resolution

The pallet resolves dynamic amounts through `AmountResolution`:

- `Fixed`
- `PercentageOfCurrent`
- `PercentageOfTrigger`
- `PercentageOfLastFunding`
- `AllBalance`

Resolution policy is task-bound in code:

- `PreserveSpend`: applies to Transfer, SplitTransfer, exact-input swap, liquidity add/remove, Stake, and DonateLiquidity; computes one spend ceiling as adapter-visible balance minus reserved future User fees for the native fee asset and minus the asset's minimum balance; `Fixed`, every percentage basis, `SplitTransfer` total, and `AllBalance` must stay within that ceiling; DonateLiquidity resolves only declared `asset_a`, with `asset_b` derived by its adapter
- `ExpendableSpend`: consume available amount where task allows
- `Mint`: amount interpreted in mint context
- `Unstake share spend`: `Fixed`, `PercentageOfCurrent`, `PercentageOfTrigger`, and `AllBalance` resolve against `StakingOps::share_balance(position_asset)` with full share withdrawal allowed; `PercentageOfLastFunding` reads the snapshot keyed by `StakingOps::share_asset(position_asset)`

Resolution outcomes are deterministic:

- `Resolved(value)`
- `Skipped`
- `FundingUnavailable`

`FundingUnavailable` is a deterministic non-terminal skip outcome for both actor classes; it covers missing/zero tracked snapshots, tracked-balance overspend, staking-share overspend, and any preserve-spend resolution that would cross the minimum-balance ceiling, while untracked assets remain `SnapshotUnavailable`. On User runs, this outcome releases the skipped step's unused execution-fee reservation before later steps resolve, matching other non-executable paths and the close-tail executor. Multi-amount tasks resolve every field before dispatch and select `FundingUnavailable > Skipped > Executable` independently of field order. Pallet boundary tests cover Fixed, current/trigger/last-funding percentages, split totals, and `AllBalance` against native, sufficient-asset, and staking-share surfaces. Unstake last-funding plans fail validation when the runtime adapter cannot expose a transferable share asset; the DEOS adapter binds native/local/foreign position keys to live staking share state and receipt assets.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier steps in the same execution plan remain committed.

### DEX Adapter Realization

`TmctolDexOps::swap_exact_in` now derives `min_out` from Axial Router's caller-aware `quote_exact_input`, so the quote includes the caller's router-fee status and the same maximum-output mechanism selection used by execution; zero tolerance binds execution to that quote. For exact output, AAA passes `max_amount_in = transferable input balance - reserved future User fees - asset minimum`. The adapter searches the caller-aware quote surface with at most `Balance::BITS` binary-search steps, computes `quoted_max_in = required_in + ceil(tolerance × required_in)`, rejects when that bound exceeds capacity, and executes only `required_in`. Runtime regressions cover fee-inclusive zero-tolerance exact input, tolerance-cap rejection without mutation, minimal exact-output input, and a User exact-output plan funded exactly for required input, later-step fees, and the preserved minimum.

---

## Scheduler Architecture

### Hook Separation

- `on_initialize`:
  - bounded scheduler bookkeeping only
  - no cycle execution
- `on_idle`:
  - bounded wakeup + ingress housekeeping
  - bounded queue-driven cycle execution using remaining `on_idle` weight

### Admission Gates

A cycle is admitted only when all checks pass:

1. actor is ready (`trigger`, cooldown, pause/breaker/window checks)
2. per-block execution cap (`MaxExecutionsPerBlock`) not exceeded
3. a two-dimensional `WeightMeter` can consume cached `cycle_weight_upper` plus any predictive close-tail bound without exceeding RefTime or ProofSize
4. for User AAA: fee preflight against cached `cycle_fee_upper` and `MinUserBalance`

`cycle_weight_upper` and `cycle_fee_upper` are stored per actor in `ActorHot` and refreshed on create/update execution plan, so admission does not recompute full execution-plan costs every pass.

Deferral/terminal paths:

- `InsufficientWeightBudget` → `CycleDeferred`, actor remains active
- rolled-back scheduler or post-cycle close → `CycleDeferred(CloseTransitionFailed)`, charged close-attempt weight, durable requeue, and deterministic retry of the stored terminal reason before readiness or another normal cycle; sweep-time rollback emits the same deferral, retains the cursor before that actor, and stops the pass for same-candidate retry next block
- Pre-cycle close precedence is deterministic: `WindowExpired` > `BalanceExhausted` > `FeeBudgetExhausted`
- User fee shortfall at admission → terminal `FeeBudgetExhausted` close
- Shipped cycle success means completion without an `AbortCycle`: skip-only and even all-failed-`ContinueNextStep` runs reset `consecutive_failures` and may satisfy auto-close, while an abort increments failure state and cannot auto-close; pallet coverage pins these sequences together with weight-defer and direct close-only event boundaries
- Post-failure close is inclusive at `consecutive_failures >= MaxConsecutiveFailures`; the admitted cycle emits its authoritative `CycleSummary` before close-tail events and `AaaClosed`, matching the existing post-success `AutoCloseNonceReached` ordering
- All close paths now use the same fully admitted `on_close_execution_plan` model: explicit `close_aaa`, lifecycle touchpoints, scheduler-triggered closes, auto-close, and sweep closure all execute the close tail instead of silently falling back to fee-free best effort
- Automatic close paths reserve the close-plan bound plus a terminal-cleanup bound ahead of time where the scheduler can predict closure (`AutoCloseNonceReached`, failure-threshold paths) and defer closure when either `on_idle` Weight dimension cannot admit the complete unit; cleanup covers bounded scans and rewrites of both run queues and one reverse-indexed full wakeup bucket plus actor/readiness/cardinality state, reverse indexes, tombstone/checkpoint, and terminal event
- With `GlobalCircuitBreaker` active, the scheduler admits neither normal cycles nor scheduler-owned automatic close tails and emits no partial cycle/close events; explicit lifecycle touchpoints and the always-metered zombie-sweep path may still enter the same terminal close contract
- User AAA close tails reserve close-time fee budget as `min(fee_native_balance, close_cycle_fee_upper)`: evaluation and execution fee collection requires both live balance and sufficient `reserved_fee_remaining` before debit; System AAA keeps the same task semantics with zero User fee charging
- Close-tail outcomes are step-addressable: `OnCloseStepSkipped` distinguishes condition, resolution, and funding skips, while `OnCloseStepFailed.kind` distinguishes evaluation fee, execution fee, condition evaluation, resolution, and adapter failures; summary counts remain aggregate
- Whole-tail skip semantics are no longer used on the current runtime path: scheduler-driven closes defer until bounded `on_idle` budget can admit the tail, and low-fund close execution degrades into classified non-blocking per-step outcomes while destruction still completes

Lifecycle lease-by-cycles is supported via `auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, actor closes with `AutoCloseNonceReached`. `set_auto_close_at_cycle_nonce` may set, shorten, extend, or clear the target, but every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`; incrementing starts from the existing target or current nonce when unset, rejects zero/overflow, and revalidates the resulting current-relative horizon.

### Fee Collection Boundary

The generic pallet collects opening, condition-evaluation, execution, and close-tail fees through one runtime-supplied `FeeCollector`. Every executable User step charges the execution fee derived from its task weight, while condition/resolution/funding skips release that reserved execution fee. `TmctolFeeCollector` atomically transfers each full charge into the Fee Sink System AAA; authorship and downstream allocation do not participate in collection. Collection failure returns to the owning create or execution path without partial debit. Pallet regressions cover opening-, evaluation-, and execution-fee failures, while runtime integration proves the payer debit and Fee Sink credit remain equal to the full collected amount.

### Queue Execution Model (Monotonic Paged FIFO)

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: an active run queue for work that can execute now, and a temporal wakeup layer for work that becomes eligible later. In the current stable line, that temporal layer is concretely represented by block-bucketed `WakeupIndex` bounded by `MaxWakeupBucketSize`; any future tree-backed or other ordered representation would now require an explicit spec/storage migration rather than being treated as a casual invisible swap.

1. **Wakeup drain**: overdue timer actors are drained from `WakeupIndex` and admitted into the active run queue; the generated dense-due model meters cursor and bucket work for `0..=MaxWakeupsPerBlock`, while each actor additionally reserves the generated full spillover-retry bound. One pass scans at most `MaxWakeupsPerBlock` actor entries and the same number of block cursors; partial buckets, unadmitted buckets, and sparse-gap catch-up retain their cursor for later blocks
2. **Ingress admission**: each matched producer event is applied independently to funding and the boolean inbox latch; the actor's readiness latch and queue membership remain bounded and may join the active run queue in the same `on_idle` pass
3. **Block-start cutoff**: after already-due wakeups materialize, the scheduler snapshots `QueueTail`; every later append receives a ticket at or beyond that cutoff and cannot execute in the current block
4. **Paged scan and admission**: the scheduler advances `QueueHead` across stale tickets under the independent scan ceiling and two-dimensional `WeightMeter`, stops at the first live ticket that cannot be admitted, and consumes a live ticket only after admission succeeds or a terminal/skip transition owns progress
5. **Execution ceiling**: admitted actors execute in FIFO order up to `MaxExecutionsPerBlock`; `last_cycle_block` prevents a second scheduler invocation or circular/self-enqueue graph from executing one actor twice in the same block, while untouched suffix entries remain physically in place without reconstruction

`ActorHot.queue_ticket` is the sole live-membership marker. Replacement, close, pause, and cancellation use actor-local invalidation; stale page entries drain lazily. Ingress, wakeup draining, paged queue operations, actor probes, normal cycles, predictive and admission-time terminal closes, and automatic zombie sweeps use two-dimensional `WeightMeter` fit checks. The close admission bound now composes actor-local queue invalidation with bounded wakeup cleanup and complete terminal state deletion rather than queue-wide scans.

### Temporal Wakeup Layer

The temporal wakeup layer is a scheduler responsibility, not just a storage map:

- It owns future eligibility for delayed timers and spillover wakeups.
- It is admitted into execution only by draining overdue entries into the active run queue.
- In the current stable line it is represented by canonical `WakeupIndex` buckets, `MinWakeupBlock`, and an actor-keyed live wakeup pointer.
- Live actors keep at most one future wakeup entry; rescheduling replaces the prior live wakeup instead of accumulating multi-bucket live state.
- Actor closure takes the actor-keyed live wakeup pointer and removes the actor from that exact bounded future bucket; it never scans the temporal index or leaves a stale entry through a supported lifecycle path.
- Close admission includes a generated 10,000-entry queue-bootstrap model as a conservative full-bucket decode/scan/rewrite proxy plus the bucket database mutation; long-horizon timer churn regressions verify immediate cleanup. Malformed orphan entries without a reverse pointer remain compatibility state and still drop lazily when due.
- Saturating every probed bucket emits/increments the existing drop observability and persists `WakeupRetryPending[aaa_id]`; each housekeeping pass selects at most `MaxSweepPerBlock` markers in canonical storage-key order, independently of `NextAaaId` and `SweepCursor`, and clears a marker only after successful scheduling or terminal cleanup.
- Each direct retry pre-admits a generated scheduler actor-probe bound plus the complete generated spillover-probe bound in both Weight dimensions before marker iteration or mutation. Close removes the marker transactionally, try-runtime rejects markers for missing actors, and a sparse-ID regression places `NextAaaId` ten million positions beyond the pending actor while still recovering it in the next funded pass.
- Any future temporal representation must preserve deterministic ordering, bounded insertion/extraction, durable saturation retry, the same overflow observability (`WakeupRescheduled`, `WakeupScheduleDropped`, `WakeupScheduleDrops`), and the same cheap close-path cost unless benchmarks justify paying more.

### Starvation Safeguard

Current implementation first admits `scheduler_on_idle_base`; if that fixed two-dimensional envelope cannot fit, `on_idle` returns zero without storage or telemetry work. The base reads paged occupancy, and a physically saturated queue reserves and attempts one generated tombstone-drain unit before producer ingress, zombie sweeping, or breaker return. A stale head therefore makes bounded progress even while the breaker remains active, while a live head stays in place. After admission, `IdleStarvationBlocks` increments only when the breaker is inactive and bounded housekeeping exhausts either RefTime or ProofSize from the remaining execution budget, resets only when both dimensions remain positive, and emits `IdleStarvationDetected` exactly once on threshold crossing. Runtime coverage keeps a durable ingress entry pending across ProofSize-only exhaustion and verifies that the same pressure reaches the telemetry threshold without beginning a drain unit.

Recovery is governance-operated (circuit breaker or parameter adjustment); no emergency cycle execution occurs in `on_initialize`.

## Trigger Subsystem

Implemented trigger variants:

- `Timer { every_blocks }`
- `OnAddressEvent { source_filter, asset_filter }`
- `Manual`

### Timer

- Timer readiness is deterministic cadence only; AAA exposes no probability field, entropy provider, secure/insecure branch, hash fallback, probability event, or probability error.
- Delayed timers derive actor-stable anti-storm jitter from `Blake2_256(aaa_id)`, and schedule validation includes the maximum reachable jitter (`window - 1`) within `MaxExecutionDelayBlocks`.
- Any future probabilistic execution requires a separate append-only trigger variant and a concrete financially secure runtime entropy contract rather than an optional field on `Timer`.

### OnAddressEvent

Inbox model uses `AddressEventInbox` key presence as the pending latch; the value is unit, so no redundant boolean, generation, or event-block metadata enters consensus state.

Filter surface:

- Source: `Any` / `OwnerOnly` / `Whitelist`
- Asset: `Any` / `Whitelist`

Each matched event applies its funding decision independently. Multiple pending events share one readiness latch and one bounded actor-queue membership without coalescing value effects.
When a cycle starts for an `OnAddressEvent` actor, the latch is consumed atomically.

Runtime ingress shape in the shipped line:

- Ingress producers go through a runtime-configured adapter interface (`AddressEventIngress` or equivalent)
- That adapter path ultimately invokes `notify_address_event*`
- Producers do not mutate `AddressEventInbox` or funding snapshots directly
- Producer scope includes transfer/mint ingress paths (asset adapters, TMC distribution, router fee routing, XCM/mint where configured)

Ingress matrix (current runtime):

- Standard signed Balances/Assets transfers to a sovereign account: transaction-extension ingress; verified debited signer supplies funding provenance and the caller pays the declared/preflight bound
- AAA task `Transfer`: `TmctolAssetOps::transfer` ingress; source is transfer sender; weight paid by actor
- AAA task `Mint`: `TmctolAssetOps::mint` ingress; source is `None`; weight paid by actor
- TMC distribution to liquidity actors: submit-first mint-output adapter; direct path keeps sender; TMC call pays primary weight
- Router fee routing to Burn Actor: submit-first `FeeManagerImpl`; direct path keeps fee payer; swap pays primary weight
- Generic top-level `pallet-assets` transfer/mint and `pallet-balances` transfer calls: producer-owned post-dispatch `AddressEventIngressExtension`; primary transfer events keep `from`, asset issuance uses `None`, and the originating extrinsic reserves the notification bound
- XCM reserve/local mint ingress: submit-first `AaaAwareAssetTransactor`; source is convertible origin or `None`; `FixedWeightBounds::UnitWeightCost` binds the generated saturated foreign-asset deposit envelope, and `MaxAssetsIntoHolding = 1` prevents one instruction from multiplying synchronous AAA ingress work until an instruction-specific multi-asset weigher ships

Fallible `notify_address_event*` remains the single canonical ingress API for inbox updates, source/asset filtering, and funding-snapshot side effects; producers preflight before value movement and propagate notification failure inside the same transaction rather than treating an overflow as best-effort. Known runtime producers now submit directly: explicit adapters cover internal protocol and XCM paths, while the transaction extension bridges generic top-level FRAME asset/balance calls whose pallets expose no transfer callback. The extension snapshots the pre-dispatch event count, inspects only events emitted by the successful bounded producer call, forwards one primary `Transfer`/`Transferred`/`Issued` event, and refunds the reserved notification envelope when no AAA recipient exists or dispatch fails. Runtime evidence now constructs signed extrinsics with the complete shipped `TxExtension` tuple and applies them through `Executive::apply_extrinsic`: matched transfer, successful non-AAA recipient refund, untracked call, and failed tracked transfer paths assert inbox, nonce, dispatch, and paid-fee behavior. Therefore an arbitrary system-event prefix cannot hide a later generic transfer; the adversarial runtime regression places more than `AaaMaxIngressEventsPerBlock` unrelated events before the producer event and still observes immediate inbox admission.

The `on_idle` ingress hook first reserves the generated fixed base before any storage access, and its ingress adapter reserves the one-read generated probe before reading ring length, then reserves one complete generated drain unit per admitted event. It drains only the durable bounded producer-owned ring; runtime event-vector prefix scanning is not a supported ingress path. Producer-owned adapters submit each concrete event position once, and no content fingerprint coalesces distinct identical transfers. Durable enqueue checks ring and funding capacity before mutation, applies accepted funding-batch mutation synchronously, and stores the event only for delayed trigger/inbox delivery; drain never replays funding. `LastIngressIngestBlock` and `AaaMaxIngressEventsPerBlock` bound one drain pass, while `IngressOverflowHead/Len/Slots` retains trigger work for later blocks. Each drained unit reserves RefTime and ProofSize before mutation. New producer families must add a direct adapter, transaction-extension case, or a transactional originating-path enqueue with observable saturation/overflow failure before activation.

Funding state follows typed provenance rather than a dedicated value-transfer call; Mutable actors update policy through call `7`:

- User AAA defaults to `OwnerOnly`; standard transfers from the verified owner or an explicitly configured signed allowlist may update tracked funding batches, while rejected and source-less deposits remain spendable balance-only donations.
- System AAA defaults to `RuntimePolicy`; the reference runtime's launch matrix contains no authorized actor/source pairs and therefore denies signed, internal-protocol, and XCM funding provenance by default. Downstream runtimes must add explicit pairs to `FundingAuthority`; Mutable actors may instead opt into `AnySource`, which still never upgrades missing provenance.
- The first accepted tracked transfer sets the batch `amount`; later accepted transfers checked-add into `pending_amount` without changing the armed amount. Funding timestamps were removed because promotion and amount resolution never consume them. Producer preflight and transactional adapters reject `FundingBatchOverflow` before value movement, expired actors accept balance only, and successful cycle completion promotes pending funding.
- `FundingSourcePolicyUpdated`, `FundingBatchActivated`, `FundingBatchPendingAccumulated`, and `FundingBatchPromoted` expose bounded state transitions for indexers and operators.

### Manual

`manual_trigger` sets `manual_trigger_pending = true` only for eligible unpaused actors: paused calls fail with `AaaPaused`, and System Immutable calls fail with `ImmutableAaa`. It is cleared when a cycle starts and preserved across deferrals.

---

## Storage Topology

Primary storage follows explicit owners. Section 13's stable behavioral stores constrain compatibility, while bounded scheduler and ingress machinery remains replaceable implementation state. No synchronized readiness mirror remains.

- `NextAaaId`: monotonic AAA id allocator
- `ActorHot`: active/paused identity, lifecycle, counters, pending readiness, eligibility anchors, live queue ticket, optional User queue-mutation block guard, and compact admission bounds; the reference runtime maximum encoded value is 169 bytes
- `ActorProgram`: active schedule/window plus bounded run/close plans; the reference runtime maximum encoded value is 8,676 bytes
- `ActorFunding`: active-only funding-source policy, bounded tracked-asset set, `funding_snapshots[asset] = FundingBatch { amount, pending_amount }`, and canonical `has_pending_funding`; ingress, promotion, and policy mutation touch this store without rewriting hot/program state, promotion skips batch traversal when false, and try-state reconciles the indication against every bounded batch
- `DormantAaaIdentities`: identity-only records with no executable or scheduler state
- `ActorIdentityCount`: transactionally maintained O(1) cardinality across `ActorHot` plus `DormantAaaIdentities`, bounded by `MaxActorIdentities`
- `ActiveAaaCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against `ActorHot`, `ActorProgram`, and `ActorFunding`
- `QueueHead` / `QueueTail` / `QueuePages`: active monotonic paged FIFO with append, block-start cutoff, exact-head admission/consume, actor-local live-ticket dedup/invalidation, tombstone draining, full-page reclamation, and empty partial-tail alignment. Page entries contain only `aaa_id`, with logical tickets derived from page/slot, so replacement tickets leave old entries physically distinguishable as tombstones without page rewrites.
- `WakeupIndex` / `MinWakeupBlock` / `ScheduledWakeupBlock`: time-ordered overdue wakeup layer; `WakeupRetryPending`: actor-keyed durable retry marker when the bounded spillover horizon is saturated
- `ActiveActorLimit`: governance-configurable operational active cap
- `OwnerSlotMask`: user owner-slot occupancy bitmask; System AAA never consumes it
- `SovereignIndex`: reverse index from sovereign account to active or dormant `aaa_id`; custody-only accounts intentionally have no entry
- `ClosedSystemAaaIds`: closed System AAA id tombstones retaining the actor's mutability; only tombstones recorded as Mutable permit governance reopen
- `AddressEventInbox`: event-trigger pending latch per actor
- `IngressOverflowHead/Len/Slots`: implementation-owned bounded O(1) carry-over ring for over-cap ingress
- `LastIngressIngestBlock`: implementation-owned once-per-block durable-ring drain coordination; producer adapters own concrete event-position uniqueness
- `GlobalCircuitBreaker`: global scheduler halt flag
- `IdleStarvationBlocks`: starvation detector
- `SweepCursor`: zombie sweep cursor

### Pre-fork storage baseline

The AAA `0.7.x` pre-launch line supports fresh genesis only; it does not claim an in-place state upgrade from `0.6.x`. Pallet genesis writes storage version `1`, and runtime integration asserts that the current and on-chain versions agree on a fresh chain. No historical `OnRuntimeUpgrade` bridge ships on this pre-fork line; downstream live forks own bounded versioned migrations after launch.

## Lifecycle State Machine

The implementation separates identity-only dormancy from active execution:

```text
Created Dormant ⇄ Active → Ready → Admitted → Running → Completed/Deferred/Failed → TerminalPending → Closing → Closed
```

`activate_aaa` accepts typed `ProgramInput` and validates the schedule/window, run plan, explicit close plan, funding policy, tracked assets, cached bounds, class restrictions, active capacity, and production idle envelope before creating matching `ActorHot`, `ActorProgram`, and `ActorFunding` entries for a Mutable identity; `ProgramInput::Dormant` is rejected on activation. `deactivate_aaa` clears queues, wakeups, inbox, funding, cycle, and fee state while preserving identity, owner slot, sovereign address, and balances. Active and dormant creation paths now normalize into the same typed internal program boundary, while the legacy creation/reopen metadata still supplies separate schedule/run-plan arguments and class-derived close/funding defaults. Converting that external metadata to `ProgramInput` remains open.

Runtime interpretation:

- `Normal cycle`: scheduler-owned `execution_plan` run; increments the stored admitted-cycle count before events, so a new actor's first run emits nonce `1` and the run from `u64::MAX - 1` emits and executes nonce `u64::MAX`; a later attempt at stored exhaustion executes no normal steps or cycle events and instead closes User AAA or pauses System AAA
- `Close tail`: terminal `on_close_execution_plan` run; does not increment `cycle_nonce`; emits close-tail events followed by `AaaClosed`
- `Lifecycle touch`: extrinsics such as `manual_trigger`, `pause_aaa`, `permissionless_sweep`, and plan/schedule updates may detect terminal state before their normal mutation path; ordinary deposits into expired/closed sovereign addresses remain balance-only

Both lowest-free-slot and exact-slot User creation now accept the complete typed `ProgramInput`, including an explicit close plan and funding policy for Active programs or no program for Dormant identities. Fresh System creation and Mutable System reopen now accept the complete typed Active or Dormant `ProgramInput`; Active programs carry their explicit close plan and funding policy through admission without class-derived metadata defaults. Mutable actors can replace an installed close plan through `update_on_close_execution_plan`; Immutable actors fix their admitted close plan for actor lifetime. User actors cannot admit `Mint` in either the normal plan or close plan. System Immutable actors are hard protocol anchors at the control boundary: no runtime extrinsic, including governance/root, can mutate, pause, manually trigger, close, or reopen them. Mandatory runtime-owned terminal transitions remain distinct from that control guard, so an Immutable actor reaching the consecutive-failure threshold executes its close tail and leaves an Immutable tombstone that governance cannot reopen; only a runtime upgrade can restore or alter that anchor.

Scheduler hygiene follows the bounded liveness matrix in the specification: one `next_eligible_at` calculation combines the applicable admitted-run cooldown, timer cadence plus actor-stable jitter, and window-start terms; execution-created late enqueues merge into next-block queue state only when eligibility reaches the next block; later eligibility receives exactly one wakeup; Manual and AddressEvent signals omit timer cadence while retaining cooldown/window gates; paused timer actors consume no queue continuation after a due wakeup and resume re-primes them from the same effective eligibility; pending non-timer actors likewise re-prime on resume; and closed/missing stale queue or wakeup entries are ignored deterministically. Pallet regressions cover paused-pop-resume, cooldown, and pre-window orderings, while runtime integration `asset_ops_transfer_notifies_on_address_event_via_runtime_ingress_adapter` proves actor-to-actor ingress remains queued across the `on_idle` boundary.

## AAA Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](./read-model.contract.en.md) split.

### Canonical on-chain AAA projections

The current pallet already provides chain-native bounded reads for live actor and scheduler truth through:

- `actor_hot(aaa_id)` for lifecycle, identity/control, queue membership, cycle state, and cached bounds
- `actor_program(aaa_id)` for schedule/window and bounded run/close plans
- `actor_funding(aaa_id)` for funding policy, tracked assets, batches, and pending indication
- `owner_slot_mask(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` recovery and `sovereign_index(sovereign)` lookup for bounded per-owner discovery/recovery
- Deterministic `sovereign_account_id_system(aaa_id)` for System AAA addressing against the known runtime catalog
- Bounded scheduler / readiness / breaker / ingress surfaces such as `QueueHead`, `QueueTail`, `QueuePages`, `WakeupIndex`, `MinWakeupBlock`, `ScheduledWakeupBlock`, `ActiveActorLimit`, `GlobalCircuitBreaker`, `AddressEventInbox`, `IdleStarvationBlocks`, and the ingress-overflow ring state
- Live execution-side effects and bounded operational events

These are the authoritative bounded surfaces for known-actor inspection, per-owner recovery, scheduler state, and current operator observability.

### Indexed / materialized AAA views

The pallet intentionally does **not** promise these as canonical on-chain surfaces:

- Long-lived per-actor execution history
- Per-step timeline replay across many cycles
- Fleet-wide dashboards, rankings, and operator analytics across arbitrary actor sets
- Archived run logs or forensic traces beyond bounded recent on-chain observability

Those belong to events plus external indexing/materialization rather than permanent in-kernel storage.

### Current boundary for actor discovery

AAA discovery is intentionally split by use case:

- User-facing recovery/discovery is chain-native only within the bounded owner-slot space: read `owner_slot_mask(owner)`, derive the occupied sovereign accounts, and resolve them through `sovereign_index`
- System AAA discovery is chain-native for the known runtime catalog because `aaa_id` values and sovereign derivation are deterministic
- Arbitrary fleet-wide discovery across all actors is still an indexed/materialized view unless a future bounded runtime projection is added

## Extrinsics (Implementation Surface)

| Call | Extrinsic | Notes |
| --- | --- | --- |
| `0` | `create_user_aaa` | fee; complete Active or Dormant program input; no User `Mint` |
| `1` | `create_user_aaa_at_slot` | exact slot; same complete Active or Dormant input |
| `2` | `create_system_aaa` | governance origin; explicit mutability and complete Active or Dormant program input |
| `3` | `reopen_system_aaa` | reopen a closed Mutable System id with a complete Active or Dormant program input |
| `4` | `pause_aaa` | mutable actors only |
| `5` | `resume_aaa` | mutable actors only |
| `6` | `manual_trigger` | set flag and enqueue/schedule |
| `7` | `update_funding_source_policy` | mutable actors; keeps existing batches |
| `8` | `close_aaa` | close tail, then destruction in place |
| `9` | `update_schedule` | mutable actors only |
| `10` | `set_global_circuit_breaker` | breaker control |
| `11` | `permissionless_sweep` | liveness touchpoint, no normal cycle |
| `12` | `update_execution_plan` | mutable actors only, re-derive assets |
| `13` | `set_active_actor_limit` | governance operational cap tuning |
| `14` | `permissionless_sweep_many` | bounded batch touchpoint, no direct enqueue |
| `15` | `set_auto_close_at_cycle_nonce` | set/clear cycle lease with horizon checks |
| `16` | `increment_auto_close_nonce` | extend cycle lease, checked and bounded |
| `17` | `update_on_close_execution_plan` | replace fully admitted close-tail plan |
| `18..=20` | reserved | retired transitional dormant creation calls; canonical User/System creation accepts `ProgramInput::Dormant` |
| `21` | `activate_aaa` | typed Active program with explicit schedule, plans, funding policy, and admission validation |
| `22` | `deactivate_aaa` | remove program/scheduler state while preserving identity and balances |

Calls `4`, `5`, `6`, `7`, `8`, `9`, `12`, `15`, `16`, `17`, `21`, and `22` use the class-specific control authority: signed owner for User actors, signed owner or governance for System actors. Active-only calls reject dormant identities; `close_aaa` handles either lifecycle.

---

## Runtime Adapters (TMCTOL Binding)

Runtime binds `pallet-aaa` in `runtime/src/configs/aaa_config.rs`:

- `FundingAuthority = DeosFundingAuthority`
  - receives only `RuntimePolicy` decisions after pallet-owned policy evaluation
  - defaults deny because the reference launch matrix contains no authorized actor/source pairs
- `AssetOps = TmctolAssetOps`
  - Native: `pallet-balances`
  - Local/Foreign: `pallet-assets`
  - `balance()` is adapter-visible transferable balance; AAA derives `spendable_balance` by subtracting only transient native fee reservation
  - includes `minimum_balance` / `can_deposit` semantics
  - exposes only queries used by portable AAA execution; total issuance remains a runtime-local ledger query
- `DexOps = TmctolDexOps`
  - swaps via Axial Router (`execute_swap_for`)
  - liquidity via `pallet-asset-conversion`
  - exposes only task execution methods; the System AAA genesis builder reads reserve depth directly from the runtime AMM
- `StakingOps = TmctolStakingOps`
  - generic `Stake { asset, amount }` delegates to the runtime staking adapter for all staking assets
  - the DEOS adapter routes its native staking asset representation to `pallet-staking::stake_native(amount)` and routes other staking assets to `pallet-staking::stake(...)`
  - collator nomination and LP custody remain outside AAA task semantics; they belong to staking/runtime-specific adapter or pallet surfaces, not to the portable AAA execution-plan shape
- `LiquidityDonationOps = TmctolLiquidityDonationOps`
  - generic `DonateLiquidity { asset_a, asset_b, amount, max_ratio_error }` delegates pair-ratio, receipt-suppression, reserve-donation, and native-special-case semantics to the runtime adapter
  - AAA records only the deterministic returned `(amount_a, amount_b)` in the task event, keeping TMCTOL liquidity policy outside the portable execution-plan shape

Additional runtime bindings:

- `WeightToFee` conversion used for execution-fee charging
- Frozen pre-paged scheduler baseline at `6ec457c`: generated `scheduler_queue_bootstrap(n)` for the physical `CurrentQueue`/`NextQueue` pair charges a core model of `10,965,000 + 273,852n` RefTime plus two reads/three writes and `1,827 + 16n` ProofSize for `n = 0..10,000`; the generated maximum-value metadata records 80,002 encoded bytes for each queue. Core-model points before database-weight addition are `(n=32: 19,728,264 / 2,339)`, `(64: 28,491,528 / 2,851)`, `(128: 46,018,056 / 3,875)`, and `(10,000: 2,749,485,000 / 161,827)` as `RefTime / ProofSize`. The baseline generated-weight hash is `448f2fe1bee9d59bbdfd4cd1a85a6f20b0f90045e862f86984d2752941075b9d`; the production compressed-Wasm hash is `947ddc39ecb1e83ec32d8b1b1200865850d622aaa8def8ad43b5d15342c3e992`. These values establish comparison evidence, not the accepted 0.7.2 representation.
- Split-store production metadata replaces the former 8,807-byte maximum `AaaInstances` value (11,282-byte per-key proof estimate) with a 169-byte maximum `ActorHot` value (2,644-byte estimate), including the optional User queue-mutation block guard, and an independently loaded 8,676-byte maximum `ActorProgram` value (11,151-byte estimate). This proves physical decode/proof separation; it does not by itself claim end-to-end scheduler throughput improvement.
- Runtime `WeightInfo` regenerated with frame omni bencher 0.22 / benchmark CLI 58 against the shipped identity/active counts, dormant lifecycle, queue-cleanup, wakeup-retry, sweep, funding-policy update, fixed hook bases, close-plan-update, User fee collection, saturated ingress-aware asset transfer, and User fee-bearing close-tail topology; dedicated lifecycle classes measure dormant System creation at `45,816,000` RefTime with seven reads/five writes and 999 measured/3,629 charged proof bytes, activation at `46,725,000` RefTime with five reads/five writes and 697 measured/3,629 charged proof bytes, and deactivation at `49,658,000` RefTime with six reads/ten writes and 1,041 measured/81,487 charged proof bytes; generated storage annotations contain no retired active-list stores
- Explicit close dispatch admission takes the component-wise maximum of the compositional System/User bound and the parameterized measured User fee-bearing close-tail model
- User cycle and close-tail admission adds two generated `fee_collection` envelopes per configured step for possible evaluation and execution charging; System admission adds neither envelope
- `Transfer`/`Burn`/`Mint` bind the generated saturated `task_simple_asset_op` envelope, while `SplitTransfer` binds the generated `task_split_transfer(l)` model over `2..=8` independently notified recipients with full queue and nine-bucket spillover pressure
- XCM `UnitWeightCost` binds generated `xcm_asset_deposit` measurement of a registered foreign asset entering a saturated `OnAddressEvent` actor through registry conversion, pallet-assets mutation, funding/inbox updates, full queue, and nine-bucket spillover pressure; the fixed weigher remains sound by restricting holding to one asset
- `SwapExactIn` and `SwapExactOut` have separate generated runtime task models covering caller-aware router execution, fee routing, and ingress effects; the exact-output class additionally covers its bounded 128-step quote search
- `Stake` and `Unstake` have separate generated runtime task models covering the shipped staking pool, transferable receipt, account, position-query, and reward-touch paths
- `AddLiquidity`, `DonateLiquidity`, and `RemoveLiquidity` have separate generated runtime task models; add-liquidity covers its worst-case missing-pool branch through pool/LP creation and first liquidity provision, donation covers native receipt acquisition, balanced pool donation, and yield bridging, while removal currently resolves an LP asset through all `MaxAdapterScan` candidate pools before mutation. `MaxAdapterScan` remains in generic `Config` only because the production adapter and its benchmark share that bound; removing the scan and this temporary embedding obligation requires a runtime-owned reverse LP index.
- All runtime-backed task adapter classes use generated measurements
- Wakeup spillover/drop retry admission uses a generated runtime model parameterized by blocked buckets and binds the shipped maximum nine-bucket probe
- Dense due-wakeup draining uses a generated zero-through-64 model covering cursor/bucket mutation, scheduled-pointer removal, actor-local paged enqueue, and missing-actor probing; cursor and bucket admission use the generated zero-unit envelope, while each actor reserves the generated one-unit envelope plus the full generated spillover retry bound
- Successful-cycle funding rotation has a generated `funding_batch_promotion(a)` class over `a = 1..10`; cycle admission always reserves the runtime maximum, with `19,103,200 + 2,393,403a` RefTime, one read/write, 4,426 charged proof bytes, and measured proof growth `463 + 37a`
- Mutable plan replacement benchmarks a prepopulated `MaxFundingTrackedAssets` batch map and removes every stale entry in the measured call; the regenerated `update_execution_plan` envelope charges three reads/three writes across hot, program, and funding ownership with 1,340 measured and 12,141 estimated proof bytes
- Producer-owned transaction-extension ingress declares the generated full matched-notification envelope and refunds against a separate generated unmatched-event base; the base conservatively benchmarks the real negative recipient resolution together with a populated `SovereignIndex` proof witness, charging `14,737,000` RefTime, one database read, and 3,521 proof bytes, and refund tests retain that complete base after an unmatched successful call. The maximum measures `AnySource` authority evaluation, existing-batch checked accumulation, funding observability, inbox mutation, and nine saturated wakeup buckets at 18 reads, 14 writes, 725,713 measured proof bytes, and a charged 743,463-byte proof estimate
- Compatibility ingress uses a generated one-read probe with 1,489 charged proof bytes before ring-length access, then a trigger-only durable-ring drain unit under saturated wakeup retry at 19 reads, 16 writes, 725,605 measured proof bytes, and a 743,463-byte charged proof estimate; enqueue-time funding reservation is paid synchronously by the originating producer, and the retired event-vector scan class no longer appears in production weights
- `scheduler_on_idle_base` charges five reads/two writes and 1,493 proof bytes before breaker/queue-occupancy/marker/starvation work; `scheduler_zombie_sweep_base` charges three reads and 3,490 proof bytes before cursor/retry-prefix access. Scheduler actor-probe admission then uses a generated model covering hot/program reads, breaker/readiness evaluation, and exhausted-budget deferral; paged scan and consume weights remain separate generated units.
- The production-Wasm `50 x 20` `scheduler_paged_execute_cheap(n)` benchmark executes complete minimal one-step System cycles through the live paged scheduler for `n = 1..1,000`. Its generated model is `91,075,000 + 64,544,609n` RefTime, `4,332 + 2,729n` estimated ProofSize, `6 + 3n` reads, and `4 + 2n` writes; measured proof is `873 + 253n`. The 999-point sample completed in about 64.7 ms on the benchmark host. This establishes the full execution cost curve and supports a 1,000 defense-in-depth count ceiling, but does not promise 1,000 executions in a reference block: the scheduler still admits each complete cached cycle bound against the available two-dimensional `WeightMeter`, and runtime stress verifies only that the guaranteed reserve exceeds the retired 48-attempt ceiling.
- `update_funding_source_policy` has a dedicated generated three-read/one-write model with 12,141 charged proof bytes and 970 measured proof bytes instead of reusing execution-plan update weight; all runtime-backed AAA task, ingress, scheduler-hook, probe, and dispatch classes use generated production measurements
- The pallet mock implements the complete runtime-benchmark helper surface, including opt-in ingress-aware asset operations isolated from ordinary pallet tests; this keeps standalone `runtime-benchmarks` compilation and its 281-test suite portable while production measurement remains runtime-owned
- `FeeSink` account is the sovereign account of System AAA `aaa_id = 1` (`FEE_SINK_AAA_ID`)
- `GenesisSystemAaas = TmctolGenesisSystemAaas`

---

## Economic and Safety Controls (Runtime Values)

Selected configured bounds in the current DEOS reference runtime:

- `MaxExecutionsPerBlock = 1,000` as a defense-in-depth count ceiling; complete-plan RefTime/ProofSize admission remains the primary limiter and no global 1,000-cycle reservation exists
- `MaxActiveActors = 10,000` (compile-time hard cap checked against `ActiveAaaCount`)
- `QueuePageSize = 64` is the selected active paged-FIFO production granularity: 32/64/128 production-Wasm evidence covers single-page operations, 10,000-entry stale traversal, mixed multi-page consumption, and complete execution through the 1,000-attempt ceiling
- `MaxQueueEntriesScannedPerBlock = 10,000`, independently bounded by physical queue capacity and not aliased to the execution ceiling
- `ActiveActorLimit` (governance operational cap, `<= min(MaxActiveActors, MaxQueueLength)`)
- `MaxWakeupBucketSize = 10,000` (temporal wakeup bucket bound, decoupled from run-queue semantics)
- Runtime block-weight policy divides capacity equally between transaction dispatch and background execution: `50%` dispatch and `50%` guaranteed `on_idle` headroom; Operational extrinsics retain their priority/fee class but have no dedicated weight reserve until a concrete critical call justifies a measured allocation, and a runtime regression pins the partition plus `reserved = None`

Production-Wasm queue-operation comparison (`50` steps, `20` repeats; each cell is `RefTime / estimated ProofSize`):

| Page entries | Append existing | Append new | Consume preserve | Consume delete |
| --- | --- | --- | --- | --- |
| 32 | `27,169,000 / 4,752` | `26,191,000 / 4,566` | `21,442,000 / 4,125` | `22,908,000 / 4,117` |
| 64 | `31,499,000 / 5,179` | `27,169,000 / 4,665` | `21,511,000 / 4,125` | `22,908,000 / 4,117` |
| 128 | `37,715,000 / 5,862` | `29,543,000 / 4,670` | `21,372,000 / 4,125` | `22,769,000 / 4,117` |

These measurements establish the generated append and consume/delete models. A second production-Wasm benchmark drains `n` absent-actor tombstones over multiple pages and yields:

| Page entries | RefTime model | Estimated ProofSize model | Page keys at 10,000 entries |
| --- | --- | --- | --- |
| 32 | `20,394,000 + 2,313,709n` | `2,949 + 2,484n` | 313 |
| 64 | `20,254,000 + 2,166,703n` | `2,903 + 2,484n` | 157 |
| 128 | `20,045,000 + 2,084,347n` | `2,844 + 2,483n` | 79 |

The reusable drain primitive snapshots an explicit cutoff, stops at the first live ticket, persists exact partial progress, reclaims exhausted pages, and reports entries scanned, tombstones skipped, pages touched, and pages deleted independently. A third production-Wasm benchmark alternates stale and live tickets, drains each stale prefix, and physically consumes each live head without executing actor programs:

| Page entries | RefTime model | Estimated ProofSize model |
| --- | --- | --- |
| 32 | `22,908,000 + 12,077,660n` | `3,138 + 2,561n` |
| 64 | `23,328,000 + 12,081,541n` | `3,092 + 2,561n` |
| 128 | `23,257,000 + 12,397,562n` | `3,034 + 2,560n` |

The mixed queue-only model makes 32 and 64 effectively close in per-entry RefTime while 128 regresses, whereas all-stale traversal favors larger pages and existing-page append favors smaller pages. The final cross-candidate complete-execution benchmark uses the same `50 x 20`, `n = 1..1,000` production-Wasm protocol:

| Page entries | RefTime model | Estimated ProofSize model | Measured ProofSize model | Queue-page keys at 999 |
| --- | --- | --- | --- | --- |
| 32 | `90,306,000 + 64,246,015n` | `4,332 + 2,729n` | `875 + 254n` | 32 |
| 64 | `91,075,000 + 64,544,609n` | `4,332 + 2,729n` | `873 + 253n` | 16 |
| 128 | `89,188,000 + 64,448,091n` | `4,331 + 2,728n` | `881 + 253n` | 8 |

Complete execution differs by less than 0.5% in per-actor RefTime and has effectively identical proof growth, so actor/program/funding work dominates page granularity. The reference runtime selects 64 as the balanced minimax choice: compared with 32 it halves page-key churn and improves all-stale traversal without materially changing complete execution; compared with 128 it avoids the larger page rewrite, the roughly 20% slower existing-page append, and the mixed-scan regression while retaining half of 32's page count. No throughput claim depends on this selection, and sustained recovery remains independently bounded by scan and WeightMeter controls.
- The `50%` reserve provides `Weight::from_parts(1_000_000_000_000, 2_500_000)` and admits the guaranteed scheduler envelope (`1,061,855` proof bytes), reference genesis System AAA `0` cycle (`757,966`), and its close tail (`370,971`) together at `2,190,792` proof bytes. The envelope includes fixed hook/probe, bounded baseline zombie-scan, queue, wakeup-cursor, and actor-probe work, not every optional ingress-drain, heavyweight wakeup-retry, or sweep-time terminal-close unit; saturated durable housekeeping therefore converges across blocks and may defer an actor cycle
- Runtime binds `GuaranteedOnIdleWeight` directly to the 50% reserve. Genesis construction asserts the contract, create/reopen reject before fee collection or mutation, and both plan-update paths validate the prospective run/close pair; `ExecutionPlanExceedsOnIdleBudget` reports either-dimensional rejection
- Every reference genesis actor fits the guaranteed scheduler envelope, its cycle, and close tail under that gate; a runtime regression checks the full set. Exact-reserve stress regressions prove mixed zero-amount staking/transfer FIFO carry-over with nonce spread `<= 1`, no starvation or failed steps, eventual drain of a maximum address-ingress batch while an XCM deposit trigger remains executable, and convergence of a maximum wakeup-retry prefix plus expired-actor cleanup while a non-empty close tail and live actors progress
- A componentwise-max 10-step cycle plus close tail still requires `18,295,309` proof bytes and is therefore unadmissible despite fitting the syntactic vector bounds
- `MaxUserExecutionPlanSteps = 3`
- `MaxSystemExecutionPlanSteps = 10`
- `MaxConsecutiveFailures = 10`
- `MaxAutoCloseNonceHorizon = 10,000`
- `MinUserBalance = max(5 * ED, ED)` (guarded)
- `MaxExecutionDelayBlocks = 52,560,000` (10y @ 6s)

These values enforce bounded execution and predictable scheduler behavior under load.

### Zombie-Spam Economic Floor (Current Baseline)

Runtime regression `zombie_spam_attack_cost_dominates_batch_cleanup_cost` checks an active-cap fill scenario against bounded cleanup:

- Scenario: fill `MaxActiveActors=10,000`
- Attacker floor per actor: `AaaCreationFee + tx_fee(create_user_aaa)`
- Cleanup floor per batch: `tx_fee(permissionless_sweep_many(MaxSweepPerBlock))`
- Calls required: `ceil(MaxActiveActors / MaxSweepPerBlock)`

Current measured baseline:

- Attacker total cost floor: `10,068,246,960,000`
- Cleanup total fee floor: `44,032,000,000`
- Cost ratio: `~228.66x` attacker cost vs bounded cleanup fee

Interpretation:

- Batch sweep keeps operational cleanup bounded and fast (`len <= MaxSweepPerBlock`)
- Creation-cost floor currently dominates bounded cleanup cost by a wide margin
- Keeper incentives are not required at current parameters; they remain an optional fallback if governance changes reduce the dominance margin
- This metric should be re-checked whenever `AaaCreationFee`, `WeightToFee`, or sweep bounds are changed

### Governance Capacity Tuning Pattern (Operational Cap vs Hard Cap)

Use a two-level model:

1. **Hard cap (`MaxActiveActors`)**
   - Compile-time upper bound enforced against the O(1) `ActiveAaaCount` state
   - Defines the absolute deterministic actor-capacity limit
   - Changed only via runtime upgrade
2. **Operational cap (`ActiveActorLimit`)**
   - Runtime storage value controlled by governance extrinsic `set_active_actor_limit`
   - Must satisfy `0 < ActiveActorLimit <= min(MaxActiveActors, MaxQueueLength)`
   - Default/effective fallback is queue-bounded, not only hard-cap bounded
   - May be raised/lowered without runtime rebuild

Recommended governance proposal workflow:

1. Measure current scheduler utilization (executions/block, rotation latency, deferrals)
2. Pick target SLA (max tolerated rotation blocks)
3. Propose new `ActiveActorLimit` with explicit rationale
4. Validate guardrails before dispatch:
   - new limit is not below current active actors
   - new limit does not exceed `MaxQueueLength`
   - throughput/fairness remain within SLA under expected load
5. Dispatch `set_active_actor_limit(new_limit)`
6. Observe for at least one full rotation window and rollback if SLA degrades

---

## Validation Coverage

Implementation is covered by:

- `template/pallets/aaa/src/tests.rs` — pallet-level unit/regression suite
- `template/runtime/src/tests/aaa_integration_tests.rs` — runtime integration suite with explicit fast/stress scheduler lanes
- `template/pallets/aaa/src/benchmarking.rs` — FRAME v2 benchmarks for all dispatchables + scheduler/close-path diagnostic benchmarks
- `aaa_0_7_2_candidate_scale_variant_indices_are_explicit` — pins candidate indices through SCALE type metadata for core `Task`, amount resolution, conditions, source/asset/funding policies, triggers, actor type/mutability, pause/close/defer/skip/failure outcomes, pallet events/errors, canonical typed creation calls (`0..=3`), retained control calls (`4..=17`), reserved retired creation indices (`18..=20`), and lifecycle calls (`21..=22`) before the post-1.0 append-only lock activates
- `aaa_0_7_2_candidate_storage_schema_is_explicit` — pins the `AAA` pallet prefix plus all 27 candidate entry names, including dormant identity and identity-count state, query modifiers, plain/map shapes, `Blake2_128Concat` map hashers, and concrete key/value SCALE types through FRAME storage metadata, making accidental schema drift fail before a live-chain migration contract exists

Coverage includes queue-scheduler fairness, fee fairness, trigger behavior, funding semantics, lifecycle transitions, and emergency starvation path.

### Operational telemetry surface

Current runtime exposes enough state/events to treat queue pressure as an operational signal rather than an inferred guess:

- Queue pressure: `QueueHead`, `QueueTail`, bounded `QueuePages`, and `ActiveActorLimit`
- Wakeup backlog: `WakeupIndex`, `MinWakeupBlock`, `WakeupRescheduled`, `WakeupScheduleDropped`, `WakeupScheduleDrops`
- Deferred-cycle rate: `CycleDeferred`
- Starvation signal: `IdleStarvationDetected`
- Sweep hygiene: `SweepBatchProcessed`

## Scheduler Performance Baseline (Release Wall-Clock)

Ignored runtime profilers measure scheduler behavior under isolated actor-set scenarios:

- `profile_scheduler_wallclock_matrix` prints actor count, block count, elapsed ms, ms/block, total executions
- `profile_scheduler_queue_wakeup_occupancy_10k` prints max queue/wakeup occupancy under 10k over-capacity pressure

Companion stress-lane checks: `scheduler_stress_lane_over_capacity_fairness_matrix`, `scheduler_stress_lane_dense_vs_sparse_topology_matrix`, `scheduler_stress_lane_sparse_topology_long_run_liveness`, `stress_10k_actors_queue_scheduler`.

Reproducible commands:

```bash
cd template
cargo test --release -p deos-runtime profile_scheduler_wallclock_matrix -- --ignored --nocapture
cargo test --release -p deos-runtime profile_scheduler_queue_wakeup_occupancy_10k -- --ignored --nocapture
cargo test --release -p deos-runtime scheduler_stress_lane_over_capacity_fairness_matrix -- --ignored
cargo test --release -p deos-runtime stress_10k_actors_queue_scheduler -- --ignored
```

Measurement environment (baseline run):

- CPU: `AMD Ryzen 7 4800H with Radeon Graphics`
- Logical CPUs: `16`

Observed results:

| Actors | Blocks | Total elapsed (ms) | ms/block | Total executions |
| ---: | ---: | ---: | ---: | ---: |
| 48 | 96 | 77.293 | 0.8051 | 4,608 |
| 100 | 150 | 122.822 | 0.8188 | 7,200 |
| 1,000 | 252 | 270.794 | 1.0746 | 12,096 |
| 10,000 | 418 | 2,127.598 | 5.0899 | 20,064 |

10K stress consistency check:

- Full release gate passes the over-capacity fairness matrix, dense/sparse topology matrix, sparse long-run liveness, 10K queue stress, and occupancy profile
- Every fairness/stress/profile test invoked by `scripts/aaa-release-gate.sh` asserts the documented starvation-free `nonce spread <= 3` oracle; the 10K occupancy profile now fails rather than merely prints when that bound regresses
- Queue/wakeup occupancy profile (`10,000`, `418`): `min_cycle_nonce = 2`, `max_cycle_nonce = 3`, `spread = 1`, `max_wakeup_backlog = 9,024`, `max_wakeup_buckets = 19`, `max_queue_occupancy = 10,000`

Temporal wakeup measurements (`scheduler_wakeup_*`):

- Dense due-drain is a generated production class over `0..=MaxWakeupsPerBlock`: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_dense_due_drain --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_dense_due_drain.rs`
- The diagnostic-only `scheduler_cooldown_ineligible_idle` case admits a Timer actor once, verifies its sole cooldown wakeup and absence of live queue membership, then measures the scheduler at an ineligible intermediate block; it remains excluded from production weights and must be regenerated before quoting post-cutover timing numbers.
- Sparse-gap recovery remains a diagnostic-only plateau probe excluded from the runtime artifact: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_sparse_gap_recovery --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_sparse_gap_recovery.rs`
- Spillover retry is a generated production class bound to `MaxSpilloverBlocks + 1` buckets; runtime constant `AaaMaxSpilloverBlocks` binds `Config::MaxSpilloverBlocks` at reference value `8`: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_spillover_probe --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_spillover_probe.rs`
- The sparse diagnostic remains the evidence gate for any future `tree`/`heap` vocabulary or storage migration; until it shows a real win against lifecycle pain points, block-bucketed `WakeupIndex` remains the reference representation.

Close-path complexity diagnostics (`on_close_execution_plan`):

- Dispatch admission: `close_dispatch_weight_upper()` adds the benchmarked `close_aaa` baseline to a component-wise maximum-plan formula (maximum User/System steps, conditions, and task RefTime/ProofSize) and the complete bounded cleanup formula; `close_aaa` and every lifecycle-touch extrinsic that may close inline declare this bound through FRAME before execution
- System diagnostic command (excluded from runtime weight artifact): `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic close_aaa_on_close_execution_plan_complex --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_close_on_close_execution_plan_complex.rs`
- User fee-bearing tail is a generated production class parameterized by User step and condition bounds: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic close_aaa_user_fee_bearing_tail --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_close_aaa_user_fee_bearing_tail.rs`
- Purpose: expose non-fee-bearing System close scaling diagnostically while binding the fee-bearing User tail to production measurements and preserving non-blocking closure semantics
- Dispatch admission composes generated dispatch/task/User-tail classes with deterministic maximum-plan and bounded terminal-cleanup upper bounds; the production artifact reflects the final shipped runtime topology
- Current evidence is considered sufficient without adding a higher-fidelity runtime-only fee-sink misconfiguration simulator: runtime integration tests already lock user/system close-tail charging semantics and scheduler deferral behavior, while pallet tests lock the non-blocking fee-sink transfer failure path directly

Interpretation:

- Scheduler overhead remains well below a 6s block budget even at 10K active actors
- 10K matrix profile aligns with long-run stress check
- Scaling trend is approximately linear over practical ranges, with visible step-up between 1K and 10K due to full active-set traversal pressure

Caveats:

- Measurements are wall-clock diagnostics, not consensus weight values
- Results depend on machine and workload profile
- PoV benchmark numbers above come from synthetic scan-only workloads and should be interpreted as scheduler-path diagnostics, not full-block end-to-end cost

## Release Gates

Current pre-release AAA scheduler gate for the DEOS reference runtime:

Repo-native wrapper: `./scripts/aaa-release-gate.sh`

1. `cargo test --release -p deos-runtime scheduler_stress_lane_over_capacity_fairness_matrix -- --ignored`
2. `cargo test --release -p deos-runtime scheduler_stress_lane_dense_vs_sparse_topology_matrix -- --ignored`
3. `cargo test --release -p deos-runtime scheduler_stress_lane_sparse_topology_long_run_liveness -- --ignored`
4. `cargo test --release -p deos-runtime stress_10k_actors_queue_scheduler -- --ignored`
5. `cargo test --release -p deos-runtime profile_scheduler_queue_wakeup_occupancy_10k -- --ignored --nocapture`

Fairness SLO for the current queue topology: high-density matrix and 10K stress runs must stay starvation-free with `nonce spread <= 3` on the documented baseline scenarios, which supply enough recurring `on_idle` budget to admit cycles. This measured SLO does not claim unconditional liveness under arbitrary zero or insufficient scheduler budget.

Local runtime-invariant dry-run wrapper: `./scripts/try-runtime-local.sh --prepare`

---

Implementation mirror for [Specification](./aaa.specification.en.md).

- `Last Updated`: July 2026
