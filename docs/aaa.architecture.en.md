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

Dormant identities retain their deterministic sovereign address, owner, Mutable System class, and genesis provider but have no `ActorHot`, `ActorProgram`, readiness, queue, wakeup, funding, fee, or cycle state. Custody-only accounts receive only a runtime-declared genesis provider so assets can enter safely; they have no generic AAA identity or control surface.

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
- `ActorProgram`: trigger/cooldown schedule, optional execution window, and run plan
- `ActorFunding`: funding policy, tracked assets, bounded armed/pending batches, and canonical pending indication

`aaa_id` exists only as each storage-map key, and dormant identity carries no timestamp. Activation or a pre-first-cycle schedule update derives first eligibility from current block, window start, cadence, and actor-stable jitter. The typed lifecycle forbids contradictory pause state.

Runtime consumers compose the split state explicitly:

- Scheduler admission, execution, lifecycle, liveness, wakeup, ingress, try-state, benchmarks, and tests combine `ActorHot + ActorProgram` through pallet-private helpers or the public `aaa_instances` Rust query helper.
- Browser and operator clients read both stores at one finalized snapshot and require both for an active actor projection.
- The temporary Rust `AaaInstances` compatibility facade is deleted; no Rust type or metadata storage prefix carries that legacy name.

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

`FundingUnavailable` is a deterministic non-terminal skip outcome for both actor classes. It covers missing or zero tracked snapshots, tracked-balance overspend, staking-share overspend, and preserve-spend resolution that would cross the minimum-balance ceiling. Untracked assets remain `SnapshotUnavailable`.

Resolution and charging follow these rules:

- A User run releases the skipped step's unused execution-fee reservation before resolving later steps, matching every non-executable cycle path.
- A multi-amount task resolves every field before dispatch and selects `FundingUnavailable > Skipped > Executable` independently of field order.
- An Unstake last-funding plan fails validation when the runtime adapter cannot expose a transferable share asset.

Pallet boundary tests cover fixed, current/trigger/last-funding percentages, split totals, and `AllBalance` across native, sufficient-asset, and staking-share surfaces. The DEOS adapter binds native/local/foreign position keys to live staking share state and receipt assets.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier steps in the same execution plan remain committed.

### DEX Adapter Realization

`TmctolDexOps` realizes swaps through the caller-aware Axial Router quote surface:

- Exact input derives `min_out` from `quote_exact_input`, including the caller's router-fee status and the same maximum-output mechanism selection used by execution. Zero tolerance binds execution to that quote.
- Exact output passes `max_amount_in = transferable input balance - reserved future User fees - asset minimum`.
- The adapter searches with at most `Balance::BITS` binary-search steps, computes `quoted_max_in = required_in + ceil(tolerance × required_in)`, rejects excess capacity, and executes only `required_in`.

Runtime regressions cover fee-inclusive zero-tolerance exact input, tolerance rejection without mutation, minimal exact-output input, and exact funding for required input, later-step fees, and the preserved minimum.

---

## Scheduler Architecture

### Hook Separation

- `on_initialize`:
  - bounded scheduler bookkeeping only
  - no cycle execution
- `on_idle`:
  - bounded temporal and queue housekeeping
  - bounded queue-driven cycle execution using remaining `on_idle` weight

### Admission Gates

A cycle is admitted only when all checks pass:

1. actor is ready (`trigger`, cooldown, pause/breaker/window checks)
2. per-block execution cap (`MaxExecutionsPerBlock`) not exceeded
3. a two-dimensional `WeightMeter` can consume cached `cycle_weight_upper` plus measured pure-cleanup weight without exceeding RefTime or ProofSize
4. for User AAA: fee preflight against cached `cycle_fee_upper` and `MinUserBalance`

`cycle_weight_upper` and `cycle_fee_upper` are stored per actor in `ActorHot` and refreshed on create/update execution plan, so admission does not recompute full execution-plan costs every pass.

Deferral/terminal paths:

- `InsufficientWeightBudget` → `CycleDeferred`, actor remains active
- pure terminal cleanup prechecks every fallible identity, funding, count, reverse-index, and User-slot invariant before mutation; no close retry or requeue state exists
- Pre-cycle close precedence is deterministic: `WindowExpired` > `BalanceExhausted` > `FeeBudgetExhausted`
- User fee shortfall at admission → terminal `FeeBudgetExhausted` close
- Shipped cycle success means completion without an `AbortCycle`: skip-only and even all-failed-`ContinueNextStep` runs reset `consecutive_failures` and may satisfy auto-close, while an abort increments failure state and cannot auto-close; pallet coverage pins these sequences together with weight-defer and direct close-only event boundaries
- Post-failure close is inclusive at `consecutive_failures >= MaxConsecutiveFailures`; an admitted cycle emits its authoritative `CycleSummary` before pure cleanup emits `AaaClosed`, matching post-success `AutoCloseNonceReached` ordering
- Explicit, automatic, lifecycle-touch, dormant, and sweep paths share one pure cleanup routine: no task, condition, fee, funding promotion, sovereign-balance movement, or shared queue/wakeup scan occurs
- Removing `ActorHot` lazily invalidates its ticket and wakeup pointer; bounded stale records converge through ordinary page draining
- Every bounded window schedules exact terminal readiness at `end + 1` through `ActorHot.terminal_at`; trigger and terminal readiness share one live wakeup pointer and retain the earlier target
- Paused actors remain hot-only before terminal time and load `ActorProgram` only when closure is due
- With `GlobalCircuitBreaker` active, normal cycles and scheduler-owned terminal cleanup defer; bounded housekeeping plus explicit lifecycle/sweep cleanup remain available

Lifecycle lease-by-cycles is supported via `auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, actor closes with `AutoCloseNonceReached`. `set_auto_close_at_cycle_nonce` may set, shorten, extend, or clear the target, but every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`; incrementing starts from the existing target or current nonce when unset, rejects zero/overflow, and revalidates the resulting current-relative horizon.

### Fee Collection Boundary

The generic pallet collects opening and per-step User fees through one runtime-supplied `FeeCollector`; terminal cleanup charges no AAA fee.

- Conditions and task preparation run read-only before collection determines the step outcome.
- Every attempted User step invokes `FeeCollector` at most once: condition/resolution/funding non-execution charges evaluation-only, while an executable step charges evaluation plus generated execution fee together.
- Collection failure emits deterministic `StepFailed` without task dispatch or partial debit. Adapter failure after successful collection rolls back task-local effects but retains the one combined charge.
- `ContinueNextStep` and `AbortCycle` do not alter the selected charge or trigger another collection.
- `TmctolFeeCollector` atomically transfers each full charge into the Fee Sink System AAA; authorship and downstream allocation do not participate in collection.

Pallet regressions cover each outcome, collection failure, one-call cardinality, and task rollback. Runtime integration proves that payer debit and Fee Sink credit equal the full collected amount. With final generated database weight, one indexed RemoveLiquidity step resolves to `978,587,000 / 8,817`, a `2,007,828,696` fee upper bound; a three-step User plan reserves `6,023,486,088`.

### Queue Execution Model (Monotonic Paged FIFO)

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: a monotonic paged FIFO for work that can execute now and an exact paged temporal layer for later eligibility. Distinct wakeup blocks use a paged minimum heap; same-block actors occupy linked fixed-capacity pages.

1. **Wakeup drain**: the cursor exposes the earliest due block without scanning sparse gaps; one admitted unit consumes one slot, preserves a partial bucket at the same minimum, and either appends live readiness to the active FIFO or lazily discards a stale pointer
2. **Ingress admission**: each matched producer call applies funding independently and sets the unified boolean signal latch; actor-local queue/wakeup membership remains bounded and may join the active run queue in the same `on_idle` pass
3. **Block-start cutoff**: after already-due wakeups materialize, the scheduler snapshots `QueueTail`; every later append receives a ticket at or beyond that cutoff and cannot execute in the current block
4. **Paged scan and admission**: the scheduler advances `QueueHead` across stale tickets under the independent scan ceiling and two-dimensional `WeightMeter`, stops at the first live ticket that cannot be admitted, and consumes a live ticket only after admission succeeds or a terminal/skip transition owns progress
5. **Execution ceiling**: admitted actors execute in FIFO order up to `MaxExecutionsPerBlock`; `last_cycle_block` prevents a second scheduler invocation or circular/self-enqueue graph from executing one actor twice in the same block, while untouched suffix entries remain physically in place without reconstruction

`ActorHot.queue_ticket` is the sole live-membership marker. Replacement, close, pause, and cancellation use actor-local invalidation; stale page entries drain lazily. Scheduler admission reserves the generated hot probe before reading `ActorHot`; paused or negative readiness consumes no `ActorProgram` proof. Only a hot-positive actor reserves the separate program/admission probe. The `50 x 20` classes measure hot probe at `10,756,000 / 3,656` with one read and program probe at `17,740,000 / 8,120` with two reads.

Wakeup draining, paged queue operations, actor probes, normal cycles, and pure terminal cleanup use two-dimensional `WeightMeter` fit checks. Direct ingress is charged at its originating producer. Close admission uses the generated worst-case User cleanup bound and performs no shared-container scan.

Scheduler accounting stays local unless it controls consensus progress. `scanned` and `executed` enforce independent per-pass ceilings; `QueueDrainStats` and `WakeupDrainStats` expose bounded operation results to callers, tests, and benchmarks without storage writes. Operators derive admission, execution, and deferral from `CycleStarted`, `CycleSummary`, `CycleDeferred`, and bounded queue/wakeup state. Loaded-actor and page-touch diagnostics remain stress instrumentation rather than permanent consensus counters.

### Temporal Wakeup Layer

The temporal wakeup layer owns future eligibility and admits it only through the active run queue. Every active actor may own at most one exact pointer; replacement and closure invalidate that slot without scanning actors or blocks. `MaxActiveActors` bounds global live wakeups, while `WakeupPageSize` controls I/O granularity rather than same-block capacity. This makes spillover buckets, placement-drop events, and actor-key retry scans unnecessary.

The production path uses the paged wakeup substrate and sparse cursor:

- `WakeupPages<(block, page_id)>` and per-block `WakeupBuckets` own the paged topology.
- `WakeupCursorPages` plus `WakeupCursorLen` provide the production paged binary min-heap over distinct wakeup blocks; each bucket owns its exact reverse `cursor_index`.
- Heap insertion and minimum removal use a `MaxActiveActors`-derived height bound, preserve contiguous cursor pages, and avoid scanning empty intermediate blocks; try-state reconciles page shape, uniqueness, ordering, and reverse indices.
- `ActorHot` owns `WakeupPointer { block, page_id, slot }`.
- Pages use optional slots, a live count, a scan cursor, and bidirectional links.
- Transactional replacement invalidates the prior exact slot, removes an emptied block from the cursor, creates the replacement bucket and cursor entry atomically, and rolls back on reverse-index mismatch; bounded neighboring-page work unlinks empty pages.
- The cursor-driven overdue worker runs before the block-start queue cutoff, peeks sparse blocks, stops before future minima, and processes one slot per admitted unit. It meters cursor lookup, page scan, queue append, and possible full-depth cursor removal before mutation; partial progress keeps the same minimum for later resumption.
- The production drain primitive bounds work by slots scanned, preserves a partial head cursor, crosses linked page boundaries, deletes exhausted pages, clears only matching live pointers, discards stale slots, and removes an exhausted bucket from the cursor in the same transaction.
- Try-state reconciles links, counts, slots, unique pointers, and active-actor capacity.

Production-Wasm `50 x 20` focused operation evidence compares candidate page sizes (`RefTime / estimated ProofSize`):

| Page entries | Append existing | Append new | Exact replacement | Unlink middle |
| --- | --- | --- | --- | --- |
| 32 | `37,645,000 / 4,827` | `39,112,000 / 4,870` | `40,508,000 / 6,646` | `50,147,000 / 10,131` |
| 64 | `39,042,000 / 5,286` | `41,486,000 / 5,258` | `40,858,000 / 6,646` | `59,017,000 / 10,623` |
| 128 | `49,169,000 / 6,033` | `52,033,000 / 5,839` | `39,321,000 / 6,646` | `58,598,000 / 11,258` |

The runtime selects `WakeupPageSize = 32`. It minimizes every page-size-sensitive operation and halves page-value rewrite granularity relative to 64. Integrated models charge four reads and three/four writes for append, seven reads/writes for cursor-coupled replacement, and five reads/writes for middle-page unlinking.

Production-Wasm `50 x 20` drain evidence at the selected page size records fixed operation samples (`RefTime / estimated ProofSize`):

| Drain case | Slots | RefTime / ProofSize | Reads / writes |
| --- | ---: | ---: | ---: |
| Partial head | 16 | `144,295,000 / 44,482` | `18 / 18` |
| Full page | 32 | `269,661,000 / 86,355` | `36 / 36` |
| Dense boundary | 33 | `284,887,000 / 89,065` | `38 / 38` |
| Stale page | 32 | `176,073,000 / 85,843` | `36 / 4` |

These fixed drain samples validate partial preservation, complete deletion, one-page boundary crossing, stale-pointer filtering, and transactional cursor removal for exhausted buckets; they do not imply block throughput.

Production-Wasm `50 x 20` cursor evidence exercises the maximum configured 10,000-block heap depth with only the pages and reverse-index buckets required by the traversed path:

| Cursor case | RefTime / ProofSize | Reads / writes |
| --- | ---: | ---: |
| Insert and full sift-up | `355,009,000 / 41,807` | `25 / 25` |
| Pop minimum and full repair | `481,354,000 / 56,199` | `34 / 26` |
| Exact removal and full repair | `434,350,000 / 55,767` | `33 / 25` |

Integrated worker evidence uses the same production-Wasm route:

| Worker case | RefTime / ProofSize | Reads / writes |
| --- | ---: | ---: |
| Partial one-slot progress | `51,474,000 / 4,285` | `8 / 5` |
| Full-depth bucket removal | `500,351,000 / 56,563` | `39 / 30` |
| Future-minimum stop | `11,734,000 / 3,906` | `2 / 0` |

The measurements prove bounded path costs, not whole-cursor throughput. Separate tests stop before mutation with either RefTime or ProofSize one unit short.

The released production artifacts use AAA weights `dd5d99ff3007e3d092cb087cd4106c03fcfbc74ee82cb9f6fdce1df13a203ae3` and compressed Wasm `be54c0be0a71342c8b94fcf74553c7a3a8d02707dab363faab965e8026c3c304`. These hashes bind the measured paths to the package-marked runtime artifact; they do not create a throughput promise.

### Starvation Safeguard

The scheduler first admits `scheduler_on_idle_base`. If that fixed two-dimensional envelope cannot fit, `on_idle` returns zero without storage or telemetry work.

Starvation handling then follows these rules:

- The base reads paged occupancy.
- A physically saturated queue reserves and attempts one generated tombstone-drain unit before breaker return.
- A stale head makes bounded progress even while the breaker remains active; a live head stays in place.
- With the breaker inactive, `IdleStarvationState` records only first starvation and alert crossing; duration derives from `since`. Positive budget or breaker activation clears state once, alerted recovery emits once, and Healthy blocks perform no telemetry write.

Runtime coverage retains actor-local readiness through ProofSize-only exhaustion and verifies telemetry without beginning an inadmissible drain unit.

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

Manual and AddressEvent ingress share `ActorHot.pending_signal` as one canonical readiness latch, so no parallel signal key, source tag, generation, or event-block metadata enters consensus state.

Filter surface:

- Source: `Any` / `OwnerOnly` / `Whitelist`
- Asset: `Any` / `Whitelist`

Each matched event applies its funding decision independently. Multiple pending events share one readiness latch and one bounded actor-queue membership without coalescing value effects.
When a cycle starts for an `OnAddressEvent` actor, the latch is consumed atomically.

Runtime ingress shape in the shipped line:

- Ingress producers go through a runtime-configured adapter interface (`AddressEventIngress` or equivalent)
- That adapter path ultimately invokes `notify_address_event*`
- Producers do not mutate `ActorHot.pending_signal` or funding snapshots directly
- Producer scope includes transfer/mint ingress paths (asset adapters, TMC distribution, router fee routing, XCM/mint where configured)

Ingress matrix (current runtime):

- Standard signed Balances/Assets transfers to a sovereign account: transaction-extension ingress; verified debited signer supplies funding provenance and the caller pays the declared/preflight bound
- AAA task `Transfer`: `TmctolAssetOps::transfer` ingress; source is transfer sender; weight paid by actor
- AAA task `Mint`: `TmctolAssetOps::mint` ingress; source is `None`; weight paid by actor
- TMC distribution to liquidity actors: submit-first mint-output adapter; direct path keeps sender; TMC call pays primary weight
- Router fee routing to Burn Actor: submit-first `FeeManagerImpl`; direct path keeps fee payer; swap pays primary weight
- Generic top-level `pallet-assets` and `pallet-balances` producers: `AddressEventIngressExtension` carries a resolved direct candidate; signed transfers keep the debited signer, while mint/force/approved calls use source-less balance-only provenance
- XCM reserve/local mint ingress: submit-first `AaaAwareAssetTransactor`; source is convertible origin or `None`; `FixedWeightBounds::UnitWeightCost` binds the generated saturated foreign-asset deposit envelope, and `MaxAssetsIntoHolding = 1` prevents one instruction from multiplying synchronous AAA ingress work until an instruction-specific multi-asset weigher ships

Fallible `notify_address_event*` remains the canonical ingress API for signal updates, filtering, and funding-snapshot side effects. Producers preflight before value movement and propagate notification failure in the same transaction; overflow never becomes best effort.

Runtime producers use two integration paths:

- Explicit adapters cover internal protocol and XCM paths.
- The transaction extension resolves fixed signed balance/asset transfers into a bounded `Pre` candidate containing actor, recipient, asset, source, and amount mode. Successful post-dispatch submits that candidate directly without reading runtime events; `transfer_all` derives actual movement from the recipient balance delta. Unmatched recipients and failed dispatches receive the generated refund.
- Asset mint, asset force-transfer, approved transfer, and Native force-transfer carry explicit source-less direct candidates. They preserve trigger delivery but remain balance-only because privileged/delegated provenance cannot honestly satisfy signed-source funding policy.

Runtime tests apply signed extrinsics through `Executive::apply_extrinsic` with the complete shipped `TxExtension`. They cover matched fixed transfer, `transfer_all`, non-AAA refund, untracked call, failed tracked transfer, and direct source-less dynamic asset producers.

The ingress contract is direct:

- Fixed signed transfers, dynamic top-level producers, and explicit internal/XCM adapters do not scan runtime events or persist a compatibility ring.
- Each successful producer call submits once; content fingerprints never coalesce distinct identical transfers, while the boolean readiness latch may coalesce trigger state independently.
- Preflight rejects funding overflow before supported value movement. The originating path pays and propagates the fallible notification rather than deferring trigger or funding mutation to `on_idle`.
- Unsupported privileged/delegated provenance remains balance-only rather than impersonating a signed funding source.

A new producer family must add a direct adapter, transaction-extension candidate, or another transactional originating-path resolver before activation.

Funding state follows typed provenance rather than a dedicated value-transfer call; Mutable actors update policy through call `7`:

- User AAA defaults to `OwnerOnly`; standard transfers from the verified owner or an explicitly configured signed allowlist may update tracked funding batches, while rejected and source-less deposits remain spendable balance-only donations.
- System AAA defaults to `RuntimePolicy`; the reference runtime's launch matrix contains no authorized actor/source pairs and therefore denies signed, internal-protocol, and XCM funding provenance by default. Downstream runtimes must add explicit pairs to `FundingAuthority`; Mutable actors may instead opt into `AnySource`, which still never upgrades missing provenance.
- The first accepted tracked transfer sets the batch `amount`; later accepted transfers checked-add into `pending_amount` without changing the armed amount. Funding timestamps were removed because promotion and amount resolution never consume them. Producer preflight and transactional adapters reject `FundingBatchOverflow` before value movement, expired actors accept balance only, and successful cycle completion promotes pending funding.
- `FundingSourcePolicyUpdated`, `FundingBatchActivated`, `FundingBatchPendingAccumulated`, and `FundingBatchPromoted` expose bounded state transitions for indexers and operators.

### Manual

`manual_trigger` sets the shared `ActorHot.pending_signal` latch only for eligible unpaused actors: paused calls fail with `AaaPaused`, and System Immutable calls fail with `ImmutableAaa`. The latch clears when a cycle starts and survives deferrals.

---

## Storage Topology

Primary storage follows explicit owners. Section 13's stable behavioral stores constrain compatibility, while bounded scheduler and ingress machinery remains replaceable implementation state. No synchronized readiness mirror remains.

- `NextAaaId`: monotonic AAA id allocator
- `ActorHot`: active/paused identity, lifecycle, counters, pending readiness, eligibility anchors, live queue ticket, exact paged wakeup pointer, direct `terminal_at`, optional User queue-mutation block guard, and compact admission bounds; the measured metadata maximum is 200 bytes
- `ActorProgram`: active schedule/window plus the bounded run plan; the measured metadata maximum is 4,655 bytes
- `ActorFunding`: active-only funding-source policy, bounded tracked-asset set, `funding_snapshots[asset] = FundingBatch { amount, pending_amount }`, and canonical `has_pending_funding`; ingress, promotion, and policy mutation touch this store without rewriting hot/program state, promotion skips batch traversal when false, and try-state reconciles the indication against every bounded batch
- `DormantAaaIdentities`: identity-only records with no executable or scheduler state
- `ActorIdentityCount`: transactionally maintained O(1) cardinality across `ActorHot` plus `DormantAaaIdentities`, bounded by `MaxActorIdentities`
- `ActiveAaaCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against `ActorHot`, `ActorProgram`, and `ActorFunding`
- `QueueHead` / `QueueTail` / `QueuePages`: active monotonic paged FIFO with append, block-start cutoff, exact-head admission/consume, actor-local live-ticket dedup/invalidation, tombstone draining, full-page reclamation, and empty partial-tail alignment. Page entries contain only `aaa_id`, with logical tickets derived from page/slot, so replacement tickets leave old entries physically distinguishable as tombstones without page rewrites.
- `WakeupPages` / `WakeupBuckets` / `WakeupCursorPages` / `WakeupCursorLen`: exact paged temporal topology and sparse minimum cursor; each live actor points to at most one page slot through `ActorHot.wakeup_pointer`
- `ActiveActorLimit`: governance-configurable operational active cap
- `OwnerSlotMask`: user owner-slot occupancy bitmask; System AAA never consumes it
- `SovereignIndex`: reverse index from sovereign account to active or dormant `aaa_id`; custody-only accounts intentionally have no entry
- `ClosedSystemAaaIds`: closed System AAA id tombstones retaining the actor's mutability; only tombstones recorded as Mutable permit governance reopen
- `GlobalCircuitBreaker`: global scheduler halt flag
- `IdleStarvationState`: sparse `Healthy | Starving { since } | Alerted { since }` starvation transition state

### Pre-fork storage baseline

The AAA `0.7.x` pre-launch line supports fresh genesis only; it does not claim an in-place state upgrade from `0.6.x`. Pallet genesis writes the reset baseline storage version `1`, and pallet, DEOS-runtime, and independent-runtime tests assert that current/on-chain versions and `try_state` agree on fresh state. No historical `OnRuntimeUpgrade` bridge ships on this pre-fork line; downstream live forks own bounded versioned migrations after launch.

### Schema delta from the frozen `0.7.1` comparison baseline

- Removed stores: `SweepCursor`, `AaaInstances`, `CurrentQueue`, `NextQueue`, `WakeupIndex`, `MinWakeupBlock`, `ScheduledWakeupBlock`, `WakeupScheduleDrops`, `WakeupRetryPending`, `QueueEpoch`, `ActorQueueEpoch`, `AaaReadiness`, `AddressEventInbox`, `IngressOverflowSlots`, `IngressOverflowHead`, `IngressOverflowLen`, `IdleStarvationBlocks`, and `LastIngressIngestBlock`.
- Split/replaced values: `AaaInstances` became `ActorHot`, `ActorProgram`, and `ActorFunding`; `FundingBatch` removed `block` and `pending_last_block`; `Timer` removed `probability`; dormant identity and sparse starvation state gained dedicated encodings.
- Added stores: `DormantAaaIdentities`, `ActorIdentityCount`, paged queue/wakeup stores, `WakeupCursorLen`, and `IdleStarvationState`. Existing owner, sovereign, active-cap, breaker, closed-System, allocator, and active-count stores retain their logical roles under the reset baseline.
- Removed public variants: `Task::Noop`, `DeferReason::CloseTransitionFailed`, `OnCloseStepFailureKind`, event variants for wakeup reschedule/drop and close-plan execution, `SecureEntropyUnavailable`, and `Error::InsecureEntropyProvider`. Current event/error indices were compacted before `1.0`; activation, deactivation, starvation recovery, identity invariants, and queue-mutation errors occupy the pinned current indices.
- Call `17` (`update_on_close_execution_plan`) was removed. Transitional dormant calls `18..=20` remain retired; calls `21..=22` own activation/deactivation. Calls `0..=3` now encode one `ProgramInput::Dormant | Active { schedule, schedule_window, execution_plan, funding_source_policy }` instead of parallel active-only arguments.
- `aaa_0_7_2_scale_variant_indices_are_explicit` and `aaa_0_7_2_storage_schema_is_explicit` pin current discriminants, call indices, all 20 storage entries, modifiers, hashers, keys, and concrete value types. The independent runtime additionally proves those split prefixes in generated runtime metadata.

## Lifecycle State Machine

The implementation separates identity-only dormancy from active execution:

```text
Created Dormant ⇄ Active → Ready → Admitted → Running → Completed/Deferred/Failed → TerminalPending → Closed
```

Lifecycle calls preserve the split-store boundary:

- `activate_aaa` accepts typed `ProgramInput` and validates schedule/window, run plan, funding policy, tracked assets, cached bounds, class restrictions, active capacity, and the production idle envelope. It then creates matching `ActorHot`, `ActorProgram`, and `ActorFunding` entries for a Mutable identity; `ProgramInput::Dormant` is rejected.
- `deactivate_aaa` clears queues, wakeups, pending signal, funding, cycle, and fee state while preserving identity, owner slot, sovereign address, and balances.

Active and dormant creation normalize into one typed internal boundary. Legacy creation/reopen metadata still supplies separate schedule/run-plan arguments and class-derived defaults; conversion of that external metadata to `ProgramInput` remains open.

Runtime interpretation:

- `Normal cycle`: scheduler-owned `execution_plan` run; increments the stored admitted-cycle count before events, so a new actor's first run emits nonce `1` and the run from `u64::MAX - 1` emits and executes nonce `u64::MAX`; a later attempt at stored exhaustion executes no normal steps or cycle events and instead closes User AAA or pauses System AAA
- `Pure close`: prechecked actor-local state/index deletion; executes no cycle or task and emits `AaaClosed` exactly once
- `Lifecycle touch`: extrinsics such as `manual_trigger`, `pause_aaa`, `permissionless_sweep`, and plan/schedule updates may detect terminal state before their normal mutation path; ordinary deposits into expired/closed sovereign addresses remain balance-only

Creation and mutability rules are explicit:

- Lowest-free-slot and exact-slot User creation accept complete typed `ProgramInput`: Active programs carry schedule, run plan, and funding policy; Dormant identities carry no program.
- Fresh System creation and Mutable System reopen accept complete Active or Dormant input without class-derived metadata defaults.
- Mutable actors may replace the run plan through `update_execution_plan`; Immutable actors fix it for actor lifetime.
- User actors cannot admit `Mint` in the run plan.
- No runtime extrinsic, including governance/root, can mutate, pause, manually trigger, close, or reopen an Immutable System actor.

Mandatory runtime-owned terminal transitions remain distinct from the control guard. An Immutable actor at the failure threshold receives the same pure cleanup and leaves an Immutable tombstone that governance cannot reopen; only a runtime upgrade can restore or alter it.

Scheduler hygiene follows the specification's bounded liveness matrix:

- One `next_eligible_at` calculation combines admitted-run cooldown, timer cadence plus actor-stable jitter, and window start.
- Execution-created late enqueues join next-block queue state only when eligibility reaches that block; later eligibility receives one wakeup.
- Manual and AddressEvent signals omit timer cadence but retain cooldown and window gates.
- Paused timer actors consume no continuation after a due wakeup; resume re-primes from effective eligibility. Pending non-timer actors re-prime likewise.
- Closed or missing stale queue and wakeup entries are ignored deterministically.

Pallet regressions cover paused-pop-resume, cooldown, and pre-window ordering. Runtime integration proves actor-to-actor ingress remains queued across the `on_idle` boundary.

## AAA Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](./read-model.contract.en.md) split.

### Canonical on-chain AAA projections

The current pallet already provides chain-native bounded reads for live actor and scheduler truth through:

- `actor_hot(aaa_id)` for lifecycle, identity/control, queue membership, cycle state, and cached bounds
- `actor_program(aaa_id)` for schedule/window and bounded run plan
- `actor_funding(aaa_id)` for funding policy, tracked assets, batches, and pending indication
- `owner_slot_mask(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` recovery and `sovereign_index(sovereign)` lookup for bounded per-owner discovery/recovery
- Deterministic `sovereign_account_id_system(aaa_id)` for System AAA addressing against the known runtime catalog
- Bounded scheduler / readiness / breaker surfaces such as `ActorHot.pending_signal`, `ActorHot.wakeup_pointer`, `QueueHead`, `QueueTail`, `QueuePages`, paged wakeup stores, `ActiveActorLimit`, `GlobalCircuitBreaker`, and `IdleStarvationState`
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
| `8` | `close_aaa` | prechecked pure destruction in place |
| `9` | `update_schedule` | mutable actors only |
| `10` | `set_global_circuit_breaker` | breaker control |
| `11` | `permissionless_sweep` | liveness touchpoint, no normal cycle |
| `12` | `update_execution_plan` | mutable actors only, re-derive assets |
| `13` | `set_active_actor_limit` | governance operational cap tuning |
| `14` | `permissionless_sweep_many` | bounded batch touchpoint, no direct enqueue |
| `15` | `set_auto_close_at_cycle_nonce` | set/clear cycle lease with horizon checks |
| `16` | `increment_auto_close_nonce` | extend cycle lease, checked and bounded |
| `17` | removed | retired pre-launch close-plan mutation |
| `18..=20` | reserved | retired transitional dormant creation calls; canonical User/System creation accepts `ProgramInput::Dormant` |
| `21` | `activate_aaa` | typed Active program with schedule, run plan, funding policy, and admission validation |
| `22` | `deactivate_aaa` | remove program/scheduler state while preserving identity and balances |

Calls `4`, `5`, `6`, `7`, `8`, `9`, `12`, `15`, `16`, `21`, and `22` use the class-specific control authority: signed owner for User actors, signed owner or governance for System actors. Active-only calls reject dormant identities; `close_aaa` handles either lifecycle.

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
- Frozen pre-paged baseline `6ec457c`: `scheduler_queue_bootstrap(n)` charges `10,965,000 + 273,852n` RefTime, two reads/three writes, and `1,827 + 16n` ProofSize for `n = 0..10,000`; each physical queue has an 80,002-byte maximum value.
- Frozen baseline sample points (`RefTime / ProofSize`, before database weight): `n=32` is `19,728,264 / 2,339`; `64` is `28,491,528 / 2,851`; `128` is `46,018,056 / 3,875`; `10,000` is `2,749,485,000 / 161,827`.
- Frozen baseline hashes: generated weights `448f2fe1bee9d59bbdfd4cd1a85a6f20b0f90045e862f86984d2752941075b9d`; compressed Wasm `947ddc39ecb1e83ec32d8b1b1200865850d622aaa8def8ad43b5d15342c3e992`. These values provide comparison evidence, not the accepted `0.7.2` representation.
- Split-store production metadata replaces the former 8,807-byte `AaaInstances` maximum with a 200-byte `ActorHot`, including exact wakeup, terminal readiness, and compact funding counts, plus an independently loaded 4,655-byte `ActorProgram`. This proves physical decode/proof separation; it does not by itself claim end-to-end throughput.
- Runtime `WeightInfo` uses frame omni bencher 0.22 / benchmark CLI 58 against shipped lifecycle, scheduler, ingress, funding, fee, and pure-close topology; generated storage annotations contain no retired active-list stores.
- Lifecycle benchmark evidence: dormant System creation is `45,816,000` RefTime, seven reads/five writes, and 999 measured/3,629 charged proof bytes.
- Lifecycle benchmark evidence: activation is `46,725,000` RefTime, five reads/five writes, and 697 measured/3,629 charged proof bytes; deactivation is `49,658,000` RefTime, six reads/ten writes, and 1,041 measured/81,487 charged proof bytes.
- Pure-close production-Wasm `50 × 20` measurement prices the worst-case User branch at `66,420,000 / 8,120`, seven reads, and seven writes; the diagnostic System branch is `57,690,000 / 8,120`, six reads, and seven writes
- Explicit and automatic close admission use the measured User bound; pure cleanup charges no fee and executes no plan
- `Transfer`, `Burn`, and System-only `Mint` bind independent generated classes. Transfer and Mint cover possible direct ingress; Burn measures only its ledger mutation and therefore inherits no queue/wakeup proof. `SplitTransfer` retains its generated `task_split_transfer(l)` model over `2..=8` independently notified recipients.
- Full production-Wasm evidence is Transfer `159,800,000 / 8,120` with 12 reads/8 writes, Burn `23,397,000 / 3,593` with one read/write, and Mint `105,812,000 / 8,120` with 10 reads/6 writes.
- XCM `UnitWeightCost` binds generated `xcm_asset_deposit` measurement of one registered foreign asset through registry conversion, pallet-assets mutation, funding/signal updates, and paged readiness registration; the fixed weigher remains sound by restricting holding to one asset
- `SwapExactIn` and `SwapExactOut` have separate generated runtime task models covering caller-aware router execution, fee routing, and ingress effects; the exact-output class additionally covers its bounded 128-step quote search
- `Stake` and `Unstake` have separate generated runtime task models covering the shipped staking pool, transferable receipt, account, position-query, and reward-touch paths
- `AddLiquidity`, `DonateLiquidity`, and `RemoveLiquidity` have separate generated runtime task models. Add-liquidity covers missing-pool creation and first provision; donation covers native receipt acquisition, balanced donation, and yield bridging; removal resolves one runtime-owned LP reverse-index entry before mutation.
- `AxialRouter::LpPairByTokenId` owns the reference runtime's `LP token -> canonical pool pair` index outside generic AAA. Runtime adapters index internal pool creation directly, while `PoolIndexExtension` indexes successful top-level Asset Conversion pool creation or liquidity addition without event scanning; conflicting reuse fails closed.
- Full indexed removal evidence is `178,587,000 / 8,817`, eight reads and six writes, replacing the former scan-shaped 38-read / `81,150` ProofSize envelope.
- All runtime-backed task adapter classes use generated measurements
- Wakeup placement uses generated append/replacement/invalidation classes over fixed-size pages plus sparse-cursor insertion or exact removal; `MaxActiveActors` bounds global live entries without spillover or drop/retry state
- Due-wakeup work uses separate generated partial-unit, full-depth removal, and future-minimum classes; each admitted slot reserves its exact queue append and possible cursor repair before mutation
- `ActorHot` stores bounded tracked/pending funding counts while `ActorFunding` retains full batches. Funding-free cycles skip the funding read and reserve no promotion work; successful cycles add `funding_batch_promotion(K)` only for actual pending `K`, including a zero-cost `K = 0` path.
- Mutable plan replacement benchmarks a prepopulated `MaxFundingTrackedAssets` batch map and removes every stale entry in the measured call; the regenerated `update_execution_plan` envelope charges three reads/three writes across hot, program, and funding ownership with 1,340 measured and 12,141 estimated proof bytes
- Producer-owned transaction-extension ingress declares the matched notification envelope and refunds against an unmatched base. The negative path includes one populated proof witness plus the absent recipient lookup: `15,365,000 / 6,052`, two reads.
- The matched ingress maximum covers source evaluation, checked batch accumulation, funding/signal mutation, queue saturation, and exact paged wakeup registration: `88,281,000 / 8,120`, nine reads, and six writes.
- `scheduler_on_idle_base` is `15,296,000 / 1,493`, four reads, and one write after compatibility ingress removal. Actor admission uses separate generated hot-only and program-positive probes; paged scan and consume weights remain independent generated units.
- Production-Wasm `50 x 20` `scheduler_paged_execute_cheap(n)` runs complete minimal one-step System cycles for `n = 1..1,000`. Its model is `91,075,000 + 64,544,609n` RefTime, `4,332 + 2,729n` estimated ProofSize, `6 + 3n` reads, and `4 + 2n` writes; measured proof is `873 + 253n`.
- The 999-point sample took about 64.7 ms on the benchmark host. This cost curve supports a 1,000 defense-in-depth count ceiling, not a promise of 1,000 reference-block executions: the two-dimensional `WeightMeter` still admits each complete cached cycle bound, and runtime stress proves only that guaranteed reserve exceeds the retired 48-attempt ceiling.
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
- `WakeupPageSize = 32` is temporal I/O granularity; `MaxActiveActors = 10,000` independently bounds global live wakeups and `MaxWakeupsPerBlock` bounds per-pass slot work
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

Complete execution differs by less than 0.5% in per-actor RefTime and has effectively identical proof growth, so actor/program/funding work dominates page granularity.

The reference runtime selects 64 as the balanced minimax choice:

- Compared with 32, it halves page-key churn and improves all-stale traversal without materially changing complete execution.
- Compared with 128, it avoids the larger rewrite, roughly 20% slower existing-page append, and mixed-scan regression while retaining half of 32's page count.

No throughput claim depends on this selection. Scan and WeightMeter controls independently bound sustained recovery.
- The `50%` reserve provides `Weight::from_parts(1_000_000_000_000, 2_500_000)` for the guaranteed scheduler envelope, one admitted cycle, and measured pure cleanup. The envelope includes the fixed hook, paged queue, wakeup cursor, and actor probes; saturated housekeeping converges across blocks and may defer a cycle
- Runtime binds `GuaranteedOnIdleWeight` directly to the reserve. Genesis, create, reopen, activation, and run-plan update validate scheduler + cycle + pure cleanup in both dimensions before fee collection or mutation
- Every reference genesis actor fits that gate. Across three active genesis actors, the maximum composed RefTime requirement is actor `10` at `4,635,294,475`, and maximum ProofSize is actor `10` at `59,132`, against the reserved `1,000,000,000,000 / 2,500,000`.
- Scheduler acceptance proves strict FIFO carry-over, independent scan/execution controls, actor-local signal retention, and convergence of 10,000 exact same-block wakeups without drops
- Maximum bounded vectors do not imply admission; `ExecutionPlanExceedsOnIdleBudget` reports either-dimensional rejection
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
- `aaa_0_7_2_scale_variant_indices_are_explicit` — pins released indices through SCALE type metadata for core `Task`, amount resolution, conditions, source/asset/funding policies, triggers, actor type/mutability, pause/close/defer/skip outcomes, pallet events/errors, canonical typed creation calls (`0..=3`), retained control calls (`4..=16`), retired indices (`17..=20`), and lifecycle calls (`21..=22`) before the post-1.0 append-only lock activates
- `aaa_0_7_2_storage_schema_is_explicit` — pins the `AAA` pallet prefix plus all 20 released entry names, including split actor state, paged queue/wakeup topology, dormant identity, and identity counts, with query modifiers, map hashers, and concrete SCALE types through FRAME storage metadata
- `seeded_state_machine_preserves_cross_store_and_scheduler_invariants` — runs 32 deterministic, shrinkable/replayable operation traces over create, activate/deactivate, funding/signal, manual enqueue, pause/resume, program update, wakeup, execution, close/reopen, and User slot round-trip; after every transition it reconciles split stores/counts, queue tickets, wakeup pointers, sovereign indices, funding promotion, per-block execution, owner-slot recovery, and balance conservation

Coverage includes queue-scheduler fairness, fee fairness, trigger behavior, funding semantics, lifecycle transitions, and emergency starvation path.

### Operational telemetry surface

Current runtime exposes enough state/events to treat queue pressure as an operational signal rather than an inferred guess:

- Queue pressure: `QueueHead`, `QueueTail`, bounded `QueuePages`, and `ActiveActorLimit`
- Wakeup backlog: `WakeupCursorLen`, `WakeupBuckets.live_entries`, paged cursor minimum, and actor-local `wakeup_pointer`
- Deferred-cycle rate: `CycleDeferred`
- Starvation signals: `IdleStarvationDetected` and `IdleStarvationRecovered`
- Sweep hygiene: `SweepBatchProcessed`

Production-Wasm `scheduler_on_idle_healthy_empty` evidence is `15,784,000 / 1,493`, five reads, zero writes, with a `14,109,000 ps` minimum execution time. It measures the breaker, empty queue, Healthy starvation state, and empty wakeup cursor without actor execution.

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

- Production classes cover append, exact replacement/invalidation, partial/full/stale page drain, cursor insert/pop/exact removal, one-slot worker progress, full-depth worker removal, and future-minimum stop.
- `scheduler_wakeup_cursor_worker_remove` materializes the maximum configured cursor depth and measures page drain, reverse-index repair, bucket deletion, and active-FIFO append as one production path.
- `scheduler_wakeup_cursor_worker_partial` verifies resumable one-slot progress without cursor removal; `scheduler_wakeup_cursor_worker_future` isolates the non-mutating minimum check.
- The diagnostic-only `scheduler_cooldown_ineligible_idle` case verifies one cooldown wakeup and no active queue membership at an intermediate block; it remains excluded from production weights.

Pure-close production diagnostics:

- `close_dispatch_weight_upper()` and lifecycle admission both use generated `close_aaa()` pricing; no maximum-plan or fee envelope participates
- Worst-case User command: `./scripts/benchmarks.sh --extrinsic close_aaa --output /tmp/close_aaa.rs pallet_aaa`
- User result at production `50 × 20`: `66,420,000 / 8,120`, seven reads, seven writes
- Diagnostic System command: `./scripts/benchmarks.sh --extrinsic close_aaa_system_pure --output /tmp/close_aaa_system.rs pallet_aaa`
- System result: `57,690,000 / 8,120`, six reads, seven writes; the benchmark remains excluded from the production weight trait because the User branch component-wise dominates it
- The former queue-sized envelope was `2,143,325,000 / 86,889`, 72 reads, and 40 writes; the new result demonstrates actor-local work rather than shared-container or plan-size scaling

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
