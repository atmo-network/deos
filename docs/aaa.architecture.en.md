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
>   - System AAA: `blake2_256(PalletId, "system", aaa_id)`; storage/event `owner_slot` stays at compatibility sentinel `0` and never consumes `OwnerSlotMask`

## Executive Summary

`pallet-aaa` is the execution platform for deterministic protocol behavior in DEOS. It provides a deterministic scheduler, bounded execution model, and adapter-driven task runtime for both user-owned and system-owned actors.

In the current DEOS reference runtime, fourteen System actors are provisioned at genesis, with one additional reserved ID for deterministic Fee Sink derivation. This shipped topology currently instantiates the TMCTOL standard:

- `aaa_id = 0`: Burning Manager execution plan
- `aaa_id = 1`: reserved deterministic Fee Sink address for unified fee collection (no System AAA execution plan)
- `aaa_id = 2`: Zap Manager skeleton execution plan
- `aaa_id = 3..6`: TOL bucket actors (A/B/C/D), initialized as timer + noop
- `aaa_id = 7..9`: Treasury B/C/D actors, initialized as timer + noop
- `aaa_id = 10`: BLDR Splitter execution plan
- `aaa_id = 11`: BLDR Zap Manager skeleton execution plan
- `aaa_id = 12`: BLDR Bucket A actor, initialized as timer + noop
- `aaa_id = 13`: BLDR Treasury actor, initialized as timer + noop
- `aaa_id = 14`: Native Staking LP Farmer actor, initialized as timer + noop until the canonical `NTVE/stNTVE` pool is activated

The pallet itself is generic and runtime-agnostic. Chain-specific behavior is injected through `AssetOps`, `DexOps`, weight/fee conversion, bounds, and genesis actor specs.

### Canonical Address Catalog (Current Runtime, Full System Map)

All addresses below are deterministic for `AaaPalletId = *b"aaactor0"` (`AccountId32`, SS58 prefix 42).
This table is the full current runtime System AAA catalog, including the reserved Fee Sink address.

| Entity                        | aaa_id | Hex                                                                  | SS58                                               |
| ----------------------------- | ------ | -------------------------------------------------------------------- | -------------------------------------------------- |
| `pallet-aaa` (pallet account) | —      | `0x6d6f646c61616163746f72300000000000000000000000000000000000000000` | `5EYCAe5fiK3ZpinaPEDXwvtT6tFp5gBL16S5vyt4TYmgLaT1` |
| Burning Manager               | 0      | `0xeba61f8494ba498cb84ce3b771bc3c193dbd82f9a999153a55c383349f6e512e` | `5HPgTa8GLrmzMDktPEWmuC82WtipKSibwd9C2pUQnESn4nAv` |
| Fee Sink                      | 1      | `0xab373631522954b038699419fadc732893dff1230239bc30fbe17bf5fb12f084` | `5FwCSs6WuW2tTv7uQFRB1o4rjmPQsgE6PesjKUUbroxfzKKh` |
| Zap Manager                   | 2      | `0xb136dc3f6dba4aac24a8c9f8be3c7b20e26b08422803b6999b7cd019c4ca50ab` | `5G54dUVans8Rvnn1qdTea3fQ28osh8T7ijaWbi3gygm9sa7C` |
| TOL Bucket A (Anchor)         | 3      | `0x6f9a5aa8cd9ba27b2e69f1bac1c521d2ffde543275ebd787da11dbd131c50d25` | `5Eb32Qkj9FpPMUXZMNreJzRESQRbYQWwiKXK4zf9VXifTEqX` |
| TOL Bucket B (Building)       | 4      | `0x03699bb4549d77d91390fc161867ccd3ef97d4f305f01757708905c84cb7d882` | `5C9BNb4AoxDngwC6nzu8SEtAEbtGHiKeBjzJwgUewA9qDNL3` |
| TOL Bucket C (Capital)        | 5      | `0x313e7fb07ed6681741b54c3d421f8c261027048e2a9b0668e1058654d369de29` | `5DBGmawvmUvHAg9e2A4bcwZm3NiGX5KE5sPCKepN36SMJvfX` |
| TOL Bucket D (Dormant)        | 6      | `0xd23baab9890a6990ff23e7ad7ab9d1ad34712d7add2344917d110e3cec5b9242` | `5GpMdwY6iMiA8LRUczsZH6p9WoxN4rX15U7FJWbeqTqTrPLX` |
| Treasury B (Building)         | 7      | `0xa027809984f38031e61246efe8ad1f28ddacd9870f6bed081560089c15f9b966` | `5FghFeZDxtGWmvASpM4etxnYtreW9yamSx1Pwh1aGYkny2uv` |
| Treasury C (Capital)          | 8      | `0xcae77c85e5665e0cbe994898429478d3facf4c29a9b7539902f95ad7b3b4bf9b` | `5GekJ6zNwu6ABqhpcagnxbPmP6UtJ1gUKdvJywZKugWkCLhe` |
| Treasury D (Dormant)          | 9      | `0xc81b0eb40aea260eb09b950cfbe2c43f9be1dc73bf62cf081c376cff4bdae0ca` | `5Gb5UKWyYyyttHG3GCsyEhN2Qtb92auewWLZzPaQCvp1RHaj` |
| BLDR Splitter                 | 10     | `0x8a420d09aa8842c9075deefab7791be5e9f9471bc68baa8c926128cfc29b6962` | `5FBz5y9kWN7ArW1w5TZiCLbszGmG3FmCSx6njj9w7VEuiK8N` |
| BLDR Zap Manager              | 11     | `0x6324e98949d19dbe10162a939df82b28368bef743a14aa8ce0a3d9a02d567221` | `5EJhZc6rdqBKzZcJXfjeMwTaQvYsyTF9YJS39sWr1HEuEy17` |
| BLDR Bucket A (Anchor)        | 12     | `0xb31a379c50afe1ba1ad65f1afafaf51df1c40ed2b6c08e9faf1a1ac2caf026de` | `5G7YDX7r2L8q5Wn73dNyhp8cnbpP3sTGUcRW6Eos5Urrxax8` |
| BLDR Treasury                 | 13     | `0x3a1bedf666c4852432a75dc0099fec586a02b813acb4457c9d4b150a03bdce45` | `5DNtvy5YymuvPBM6Wk8ADHs9ggLK2gjEZoaSoeM3aHLykNKG` |
| Native Staking LP Farmer      | 14     | `0xbb27f4956462189d16c7f9e207222ce9691308c6a55bb0141f139ebe071394d2` | `5GJ6gSae5dZhxJm6EuD82gaxiLkvokMeLFMNmtuSz8htoidu` |

---

## Architecture Overview

### Design Principles

1. `Deterministic scheduling`: queue-driven double-buffer execution with deterministic ordering and explicit per-block caps
2. `Execution safety`: explicit weight and fee admission gates before cycle start
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`FeeBudgetExhausted`, `BalanceExhausted`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `Hot-state decomposition`: readiness checks use compact scheduler state to avoid full plan decode on scan path

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

`TmctolGenesisSystemAaas` currently provisions fourteen System actors plus one reserved fee-sink ID:

| Lane     | Actor               | aaa_id | Genesis schedule | Genesis execution plan                                 |
| :------- | :------------------ | -----: | :--------------- | :----------------------------------------------------- |
| Core     | Burning Manager     |      0 | `Timer(10)`      | `build_burn_execution_plan([], dust)`                  |
| Core     | Fee Sink (reserved) |      1 | —                | no actor; deterministic unified fee-collection address |
| Core     | Zap Manager         |      2 | `Timer(1)`       | `Noop` skeleton until pool activation                  |
| TOL      | Bucket A (Anchor)   |      3 | `Timer(1)`       | `Noop`                                                 |
| TOL      | Bucket B (Building) |      4 | `Timer(1)`       | `Noop`                                                 |
| TOL      | Bucket C (Capital)  |      5 | `Timer(1)`       | `Noop`                                                 |
| TOL      | Bucket D (Dormant)  |      6 | `Timer(1)`       | `Noop`                                                 |
| Treasury | Treasury B          |      7 | `Timer(1)`       | `Noop`                                                 |
| Treasury | Treasury C          |      8 | `Timer(1)`       | `Noop`                                                 |
| Treasury | Treasury D          |      9 | `Timer(1)`       | `Noop`                                                 |
| BLDR     | BLDR Splitter       |     10 | `Timer(1)`       | `build_bldr_splitter_execution_plan(BLDR, dust)`       |
| BLDR     | BLDR Zap Manager    |     11 | `Timer(1)`       | `Noop` skeleton until NTVE-BLDR pool activation        |
| BLDR     | BLDR Bucket A       |     12 | `Timer(1)`       | `Noop`                                                 |
| BLDR     | BLDR Treasury       |     13 | `Timer(1)`       | `Noop`                                                 |
| Staking  | Native Staking LP Farmer | 14 | `Timer(1)`       | `Noop` skeleton until `NTVE/stNTVE` pool activation    |

All timer schedules use `SYSTEM_AAA_COOLDOWN_BLOCKS`; all actors are `AaaType::System`, `Mutability::Mutable`, and perpetual (`schedule_window = None`).
These `aaa_id` values remain the stable recovery addresses for System AAA: closing preserves balances on the same sovereign account, and governance regains control only through `reopen_system_aaa` on that exact `aaa_id`.

### Canonical execution-plan families

The runtime keeps System AAA topology declarative. Governance evolves concrete execution plans through builder functions in `runtime/src/configs/aaa_config.rs`:

| Builder                                   | Actor(s)         | Canonical flow                                                                                        | Activation point                              |
| :---------------------------------------- | :--------------- | :---------------------------------------------------------------------------------------------------- | :-------------------------------------------- |
| `build_burn_execution_plan`               | Burning Manager  | foreign balances -> `SwapExactIn` to Native -> `Burn`                                                 | extend after foreign asset registration       |
| `build_zap_execution_plan`                | Zap Manager      | opportunistic `AddLiquidity` -> patriotic foreign-to-native swap -> LP `SplitTransfer` to TOL buckets | activate after Native/foreign pool creation   |
| `build_bucket_unwind_execution_plan`      | Buckets B/C/D    | `RemoveLiquidity` percentage -> transfer reclaimed Native + foreign to paired treasury                | activate after pool + treasury lane are ready |
| `build_bldr_splitter_execution_plan`      | BLDR Splitter    | transfer all NTVE collateral to BLDR ZM -> split minted BLDR 50/50 to BLDR ZM + BLDR Treasury         | active at genesis                             |
| `build_bldr_zm_execution_plan`            | BLDR Zap Manager | opportunistic `AddLiquidity(NTVE, BLDR)` -> transfer LP to BLDR Bucket A                              | activate after NTVE-BLDR pool creation        |
| `build_treasury_b_buyback_execution_plan` | Treasury B       | swap NTVE percentage into target asset -> `Burn` acquired balance                                     | optional governance policy lane               |
| `build_native_staking_lp_farming_execution_plan` | Native Staking LP Farmer | `$NTVE` budget -> deterministic stake split -> balanced `NTVE/stNTVE` donation without minting LP | activate after native staking pool + `NTVE/stNTVE` AMM creation |

This split keeps AAA generic: the pallet owns bounded scheduling/execution, while the current DEOS reference runtime wires the TMCTOL standard's economic composition into concrete System actors.

### Governance activation flows

Current runtime operations reduce to four repeatable flows:

1. `Foreign asset + TOL lane`
   Register foreign asset -> create Native/foreign pool -> update Burning Manager -> update Zap Manager -> optionally activate Bucket B/C/D unwind plans.
2. `BLDR lane`
   Keep BLDR Splitter live at genesis -> create NTVE-BLDR pool -> activate BLDR ZM -> optionally activate Treasury B buyback/burn policy.
3. `Native staking LP farming lane`
   Register native staking -> initialize `stNTVE` -> create and seed the `NTVE/stNTVE` AMM -> call `activate_native_staking_lp_farming`, which refuses activation until the receipt asset, staking pool, actor, and non-empty AMM are all live.
4. `Emergency controls`
   Pause single actors with `pause_aaa` when policy needs surgical intervention; use `set_global_circuit_breaker(true)` when cycle execution as a whole must stop while bookkeeping stays alive.

---

## Execution Model

### Actor Classes

| Class    | Ownership                     | Mint task allowed | Typical usage       |
| :------- | :---------------------------- | :---------------- | :------------------ |
| `User`   | Signed owner + slot namespace | No                | User automation     |
| `System` | Governance origin             | Yes               | Protocol automation |

User recovery now has an explicit slot-targeted surface: the default `create_user_aaa` path allocates the lowest free slot, while `create_user_aaa_at_slot` recreates control for a chosen slot/sovereign deterministically.

Current owner-slot representation is intentionally compact and runtime-shaped:

- `OwnerSlotMask` is stored as a `u8` occupancy bitmask for the bounded user-slot namespace
- Bits above `MaxOwnerSlots` are masked away before allocation decisions
- The current representation is little-endian in the usual `(1 << n) - 1` sense
- Default allocation walks the lowest free bit first, while the exact-slot path fails if the requested bit is already occupied

### Current Actor-State Shape

In the shipped runtime, `AaaInstances` carries one compact state object per actor with these concrete field families:

- Identity and ownership (`aaa_id`, actor class, sovereign account, owner, owner-slot sentinel/identity, mutability)
- Lifecycle control (pause state/reason, optional auto-close target, schedule, schedule window)
- Execution state (run plan, close plan, cycle nonce, last admitted cycle block, failure counter, manual-trigger flag)
- Tracked funding state (tracked asset set plus per-asset funding snapshots)
- Admission caches (`cycle_weight_upper`, `cycle_fee_upper`, plus close-tail derivation inputs)
- Creation/update timestamps

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
- `Noop`

### Amount Resolution

The pallet resolves dynamic amounts through `AmountResolution`:

- `Fixed`
- `PercentageOfCurrent`
- `PercentageOfTrigger`
- `PercentageOfLastFunding`
- `AllBalance`

Resolution policy is task-bound in code:

- `PreserveSpend`: keep native ED-safe behavior where required
- `ExpendableSpend`: consume available amount where task allows
- `Mint`: amount interpreted in mint context

Resolution outcomes are deterministic:

- `Resolved(value)`
- `Skipped`
- `FundingUnavailable`

`FundingUnavailable` is a deterministic non-terminal skip outcome for both actor classes; it covers missing/zero tracked snapshots and tracked-balance overspend, while untracked assets remain `SnapshotUnavailable`.

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
3. weight budget sufficient against cached `cycle_weight_upper`
4. for User AAA: fee preflight against cached `cycle_fee_upper` and `MinUserBalance`

`cycle_weight_upper` and `cycle_fee_upper` are stored per actor (`AaaInstance`) and refreshed on create/update execution plan, so admission does not recompute full execution-plan costs every pass.

Deferral/terminal paths:

- `InsufficientWeightBudget` → `CycleDeferred`, actor remains active
- Pre-cycle close precedence is deterministic: `WindowExpired` > `BalanceExhausted` > `FeeBudgetExhausted`
- User fee shortfall at admission → terminal `FeeBudgetExhausted` close
- Post-failure close is inclusive at `consecutive_failures >= MaxConsecutiveFailures`; post-success close is `AutoCloseNonceReached` only
- All close paths now use the same fully admitted `on_close_execution_plan` model: explicit `close_aaa`, lifecycle touchpoints, scheduler-triggered closes, auto-close, and sweep closure all execute the close tail instead of silently falling back to fee-free best effort
- Automatic close paths reserve close-tail dispatch weight ahead of time where the scheduler can predict closure (`AutoCloseNonceReached`, failure-threshold paths) and defer closure when bounded `on_idle` budget cannot yet admit the tail, rather than skipping cleanup for lack of dispatch headroom
- User AAA close tails reserve close-time fee budget as `min(fee_native_balance, close_cycle_fee_upper)`: if fee-native balance runs out during close execution, individual close steps fail/skip observably while destruction still completes; System AAA keeps the same task semantics with zero User fee charging
- Whole-tail skip semantics are no longer used on the current runtime path: scheduler-driven closes defer until bounded `on_idle` budget can admit the tail, and low-fund close execution degrades into non-blocking per-step failure instead

Lifecycle lease-by-cycles is supported via `auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, actor closes with `AutoCloseNonceReached`.

### Queue Execution Model (Bounded Double Buffer)

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: an active run queue for work that can execute now, and a temporal wakeup layer for work that becomes eligible later. In the current stable line, that temporal layer is concretely represented by block-bucketed `WakeupIndex` bounded by `MaxWakeupBucketSize`; any future tree-backed or other ordered representation would now require an explicit spec/storage migration rather than being treated as a casual invisible swap.

1. **Wakeup drain:** overdue timer actors are drained from `WakeupIndex` (bounded by `MaxWakeupsPerBlock`) and admitted into the active run queue
2. **Ingress admission:** matched address events are coalesced and may join the active run queue in the same `on_idle` pass
3. **Run queue:** the active queue starts from `CurrentQueue` plus staged `NextQueue`; actors are executed up to `MaxExecutionsPerBlock` and weight budget
4. **Carry-over staging:** deferred or leftover work is written back into bounded next-block queue storage (`CurrentQueue`, with `NextQueue` cleared at epoch end)
5. **Epoch advance:** `QueueEpoch` advances after the carry-over queue is persisted

`ActorQueueEpoch` provides deterministic dedup semantics and prevents duplicate queue entries across same-epoch enqueue attempts.

### Temporal Wakeup Layer

The temporal wakeup layer is a scheduler responsibility, not just a storage map:

- It owns future eligibility for delayed timers and spillover wakeups.
- It is admitted into execution only by draining overdue entries into the active run queue.
- In the current stable line it is represented by canonical `WakeupIndex` buckets, `MinWakeupBlock`, and an actor-keyed live wakeup pointer.
- Live actors keep at most one future wakeup entry; rescheduling replaces the prior live wakeup instead of accumulating multi-bucket live state.
- Close-path policy remains intentionally `lazy zombie-drop`: actor closure clears the live wakeup pointer and active-queue presence immediately, but future wakeup buckets are not scanned on close.
- Because close is lazy while live rescheduling is single-entry, each closed actor can leave at most one stale due-time wakeup entry; stale cleanup remains bounded by `MaxWakeupsPerBlock` once the affected bucket becomes due.
- Any future temporal representation must preserve deterministic ordering, bounded insertion/extraction, the same overflow observability (`WakeupRescheduled`, `WakeupScheduleDropped`, `WakeupScheduleDrops`), and the same cheap close-path cost unless benchmarks justify paying more.

### Starvation Safeguard

Current implementation now follows the polished `0.1.0` starvation automaton: `IdleStarvationBlocks` increments only when the breaker is inactive and bounded housekeeping leaves zero remaining `on_idle` execution budget, resets as soon as positive post-housekeeping budget exists, and emits `IdleStarvationDetected` exactly once on threshold crossing.

Recovery is governance-operated (circuit breaker or parameter adjustment); no emergency cycle execution occurs in `on_initialize`.

## Trigger Subsystem

Implemented trigger variants:

- `Timer { every_blocks, probability }`
- `OnAddressEvent { source_filter, asset_filter }`
- `Manual`

### Timer

- Deterministic cadence with optional probabilistic gate
- Probability miss is a readiness miss, not a deferred-cycle outcome
- Entropy fallback chain in implementation:
  1. `EntropyProvider::entropy(subject)`
  2. `parent_hash`
  3. previous block hash
- Current runtime posture: no dedicated external secure entropy provider is wired after local VRF retirement, so probabilistic timers rely on this fallback chain unless a future relay-beacon adapter supplies external entropy; TMCTOL does **not** currently accept the visible epoch-scale relay randomness items as that replacement and stays on previous-block-hash fallback until a real per-block protocol beacon exists
- If such a per-block relay beacon appears later, the preferred ingestion topology is still a custom parachain-system `ConsensusHook` wrapper that reads the relay proof once per block, materializes a compact relay snapshot into pallet storage, and lets AAA's runtime `EntropyProvider` derive subject-specific entropy from that snapshot later in the block

### OnAddressEvent

Inbox model uses per-AAA pending-latch state (`AddressEventInbox`):

- `is_pending`
- `generation`
- `last_event_block`

Filter surface:

- Source: `Any` / `OwnerOnly` / `Whitelist`
- Asset: `Any` / `Whitelist`

Matched events set the latch and enqueue readiness; multiple events are coalesced.
When a cycle starts for an `OnAddressEvent` actor, the latch is consumed atomically.

Runtime ingress shape in the shipped line:

- Ingress producers go through a runtime-configured adapter interface (`AddressEventIngress` or equivalent)
- That adapter path ultimately invokes `notify_address_event*`
- Producers do not mutate `AddressEventInbox` or funding snapshots directly
- Producer scope includes transfer/mint ingress paths (asset adapters, TMC distribution, router fee routing, XCM/mint where configured)

Ingress matrix (current runtime):

| Producer path                                  | Ingress wiring                                                                               | Source semantics                                                     | Weight payer                    |
| :--------------------------------------------- | :------------------------------------------------------------------------------------------- | :------------------------------------------------------------------- | :------------------------------ |
| `fund_aaa`                                     | Yes (via `AssetOps::transfer` -> ingress adapter)                                            | `source = funder account`                                            | `fund_aaa` caller               |
| AAA task `Transfer`                            | Yes (`TmctolAssetOps::transfer` -> ingress adapter)                                          | `source = transfer sender`                                           | Executing actor                 |
| AAA task `Mint`                                | Yes (`TmctolAssetOps::mint` -> ingress adapter)                                              | `source = None`                                                      | Executing actor                 |
| TMC distribution to Zap                        | Submit-first (`TolZapAdapter::transfer_to_zap_manager` -> ingress adapter), scanner fallback | direct path keeps exact sender; fallback derives from transfer event | TMC call path (primary)         |
| Router fee routing to Burning Manager          | Submit-first (`FeeManagerImpl::route_fee` -> ingress adapter), scanner fallback              | direct path keeps exact fee payer; fallback derives from event       | Router swap call path (primary) |
| Generic pallet-assets transfer/mint extrinsics | Fallback via `RuntimeAddressEventIngressHook` scan                                           | `Transferred`: `source = from`; `Issued/Deposited`: `source = None`  | Originating extrinsic path      |
| XCM reserve/local mint ingress                 | Submit-first (`AaaAwareAssetTransactor` direct) + scanner fallback                           | `source = context.origin` when convertible; else `None`              | XCM execution path/trader       |

`notify_address_event*` remains the single canonical ingress API for inbox updates, source/asset filtering, and System AAA funding snapshot side effects. The runtime ingress strategy is submit-first where hooks are available, scanner-fallback where hooks are unavailable. To prevent duplication across submit and fallback paths, AAA applies deterministic same-block dedup fingerprints. The runtime hook ingests block events at most once per block (`LastIngressIngestBlock`) with bounded scan/admission controls (`AaaMaxIngressScanEventsPerBlock`, `AaaMaxIngressEventsPerBlock`) and calibrated per-event ingestion weight. Before admission, scanner candidates are coalesced by `(aaa_id, asset, source)` to reduce duplicate processing in event-dense blocks. If admission budget is exhausted, recognized events are deferred into bounded carry-over storage (`IngressOverflowHead/Len/Slots` ring-buffer) and drained in subsequent blocks before new event scanning.

Funding snapshot policy is intentionally asymmetric:

- System AAA may refresh tracked `PercentageOfLastFunding` snapshots via `notify_address_event*`, because protocol-owned accounts are fed by deterministic runtime-managed ingress paths.
- User AAA snapshots remain explicit through `fund_aaa`; passive inbound transfers or third-party deposits must not silently rewrite user budgeting baselines.
- Expected UX flow for User AAA is: move assets first, then call `fund_aaa` when the new balance should become the snapshot baseline.

### Manual

`manual_trigger` sets `manual_trigger_pending = true` only for unpaused actors (paused calls fail with `AaaPaused`). It is cleared when a cycle starts and preserved across deferrals.

---

## Storage Topology

Primary storage in pallet implementation:

| Storage                                                   | Purpose                                                                                                                                        |
| :-------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------- |
| `NextAaaId`                                               | monotonic AAA id allocator                                                                                                                     |
| `AaaInstances`                                            | full actor state (plan, lifecycle, funding snapshots, metadata)                                                                                |
| `AaaReadiness`                                            | compact hot-state for scheduler readiness checks                                                                                               |
| `CurrentQueue`                                            | bounded run queue for current block                                                                                                            |
| `NextQueue`                                               | staged queue merged into the active run queue on the next `on_idle`                                                                            |
| `WakeupIndex` / `MinWakeupBlock` / `ScheduledWakeupBlock` | time-ordered wakeup layer for overdue actor activation (currently block-bucketed + actor-keyed live pointer, bounded by `MaxWakeupBucketSize`) |
| `QueueEpoch` / `ActorQueueEpoch`                          | deterministic queue dedup and epoch tracking                                                                                                   |
| `ActiveActorLimit`                                        | governance-configurable operational active-cap (≤ hard cap)                                                                                    |
| `OwnerSlotMask`                                           | user owner-slot occupancy bitmask; System AAA never consumes it even though storage/events expose `owner_slot = 0`                             |
| `SovereignIndex`                                          | reverse index: sovereign account → aaa_id                                                                                                      |
| `ClosedSystemAaaIds`                                      | closed System AAA id tombstones used to authorize governance reopen                                                                            |
| `AddressEventInbox`                                       | event trigger pending-latch per actor                                                                                                          |
| `IngressOverflowHead/Len/Slots`                           | bounded O(1) ring-buffer carry-over for over-cap recognized ingress                                                                            |
| `IngressSeenBlock/Set`                                    | same-block dedup fingerprints across submit + scanner paths                                                                                    |
| `GlobalCircuitBreaker`                                    | global scheduler halt flag                                                                                                                     |
| `IdleStarvationBlocks`                                    | starvation detector                                                                                                                            |
| `SweepCursor`                                             | zombie sweep cursor                                                                                                                            |

## AAA Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](./read-model.contract.en.md) split.

### Canonical on-chain AAA projections

The current pallet already provides chain-native bounded reads for live actor and scheduler truth through:

- `aaa_instances(aaa_id)` for known actor state, execution plans, lifecycle metadata, and cached bounds
- `owner_slot_mask(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` recovery and `sovereign_index(sovereign)` lookup for bounded per-owner discovery/recovery
- Deterministic `sovereign_account_id_system(aaa_id)` for System AAA addressing against the known runtime catalog
- Bounded scheduler / readiness / breaker / ingress surfaces such as `AaaReadiness`, `CurrentQueue`, `NextQueue`, `WakeupIndex`, `MinWakeupBlock`, `ScheduledWakeupBlock`, `ActiveActorLimit`, `GlobalCircuitBreaker`, `AddressEventInbox`, `IdleStarvationBlocks`, and the ingress-overflow ring state
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

| Call | Extrinsic                        | Notes                                                                                                   |
| :--- | :------------------------------- | :------------------------------------------------------------------------------------------------------ |
| `0`  | `create_user_aaa`                | charges `AaaCreationFee`, forbids `Mint`, allocates the lowest free user slot                           |
| `1`  | `create_system_aaa`              | governance/system origin                                                                                |
| `2`  | `pause_aaa`                      | mutable actors only                                                                                     |
| `3`  | `resume_aaa`                     | mutable actors only                                                                                     |
| `4`  | `manual_trigger`                 | sets pending manual trigger                                                                             |
| `5`  | `fund_aaa`                       | enforces tracked-asset funding discipline                                                               |
| `6`  | `close_aaa`                      | owner/governance close path                                                                             |
| `7`  | `update_schedule`                | mutable actors only                                                                                     |
| `8`  | `set_global_circuit_breaker`     | breaker control                                                                                         |
| `9`  | `permissionless_sweep`           | public lifecycle touchpoint; evaluates liveness only, never enqueues directly                           |
| `10` | `update_execution_plan`          | mutable actors only, tracked assets re-derived                                                          |
| `11` | `set_active_actor_limit`         | governance operational cap (`0 < limit ≤ min(MaxActiveActors, MaxQueueLength)`) tuning                  |
| `12` | `permissionless_sweep_many`      | bounded batch lifecycle touchpoint (`len <= MaxSweepPerBlock`), no direct enqueue                       |
| `13` | `set_auto_close_at_cycle_nonce`  | set/clear cycle lease target (`Option<u64>`) with horizon checks                                        |
| `14` | `increment_auto_close_nonce`     | extend cycle lease by `by` (`by > 0`, checked add, bounded horizon)                                     |
| `15` | `update_on_close_execution_plan` | configure best-effort close cleanup execution plan (same task/condition surface as main execution plan) |
| `16` | `create_user_aaa_at_slot`        | charges `AaaCreationFee`, forbids `Mint`, and binds creation to an exact user slot for recovery         |
| `17` | `reopen_system_aaa`              | governance-only reopen of a previously closed System AAA at the same `aaa_id`                           |

Calls `2`, `3`, `4`, `6`, `7`, `10`, `13`, `14`, and `15` all share the same authority gate: signed owner for both actor types, plus governance origin for System AAA only.

---

## Runtime Adapters (TMCTOL Binding)

Runtime binds `pallet-aaa` in `runtime/src/configs/aaa_config.rs`:

- `AssetOps = TmctolAssetOps`
  - Native: `pallet-balances`
  - Local/Foreign: `pallet-assets`
  - `balance()` is adapter-visible transferable balance; AAA derives `spendable_balance` by subtracting only transient native fee reservation
  - includes `minimum_balance` / `can_deposit` semantics
- `DexOps = TmctolDexOps`
  - swaps via Axial Router (`execute_swap_for`)
  - liquidity via `pallet-asset-conversion`
- `StakingOps = TmctolStakingOps`
  - generic `Stake { asset, amount }` delegates to the runtime staking adapter for all staking assets
  - the DEOS adapter routes its native staking asset representation to `pallet-staking::stake_native(amount)` and routes other staking assets to `pallet-staking::stake(...)`
  - collator nomination and LP custody remain outside AAA task semantics; they belong to staking/runtime-specific adapter or pallet surfaces, not to the portable AAA execution-plan shape

Additional runtime bindings:

- `WeightToFee` conversion used for execution-fee charging
- `TaskWeightInfo` for coarse per-task upper bounds
- `FeeSink` account derived from reserved `aaa_id = 1` (`FEE_SINK_AAA_ID`)
- `GenesisSystemAaas = TmctolGenesisSystemAaas`

---

## Economic and Safety Controls (Runtime Values)

Selected configured bounds in the current DEOS reference runtime:

- `MaxExecutionsPerBlock = 48`
- `MaxActiveActors = 10,000` (compile-time hard cap)
- `ActiveActorLimit` (governance operational cap, `<= min(MaxActiveActors, MaxQueueLength)`)
- `MaxWakeupBucketSize = 10,000` (temporal wakeup bucket bound, decoupled from run-queue semantics)
- `MinOnIdleReservePct = 10%` (runtime-level dispatchable cap preserves on_idle headroom)
- `MaxUserExecutionPlanSteps = 3`
- `MaxSystemExecutionPlanSteps = 10`
- `MaxConsecutiveFailures = 10`
- `MaxAutoCloseNonceHorizon = 10,000`
- `MinUserBalance = max(5 * ED, ED)` (guarded)
- `MaxExecutionDelayBlocks = 52,560,000` (10y @ 6s)
- `RequireSecureEntropyForProbabilisticTasks = false` (runtime currently allows probabilistic timers to fall back to previous-block-hash sampling while the project stays on a trusted collator set; if a future per-block relay beacon ever becomes acceptable, the preferred ingress path is a weight-accounted `ConsensusHook` snapshot rather than per-timer proof reconstruction)

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
   - Compile-time storage bound for `BoundedBTreeSet`
   - Defines absolute deterministic upper limit for proofs/weights
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

Coverage includes queue-scheduler fairness, fee fairness, trigger behavior, funding semantics, lifecycle transitions, and emergency starvation path.

### Operational telemetry surface

Current runtime exposes enough state/events to treat queue pressure as an operational signal rather than an inferred guess:

- Queue pressure: `CurrentQueue`, `NextQueue`, `ActiveActorLimit`
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
cargo test --release -p deos-runtime profile_readiness_hot_state_vs_fallback -- --ignored --nocapture
cargo test --release -p deos-runtime scheduler_stress_lane_over_capacity_fairness_matrix -- --ignored
cargo test --release -p deos-runtime stress_10k_actors_queue_scheduler -- --ignored
```

Measurement environment (baseline run):

- CPU: `AMD Ryzen 7 4800H with Radeon Graphics`
- Logical CPUs: `16`

Observed results:

| Actors | Blocks | Total elapsed (ms) | ms/block | Total executions |
| -----: | -----: | -----------------: | -------: | ---------------: |
|     48 |     96 |             52.209 |   0.5438 |            4,608 |
|    100 |    150 |             84.109 |   0.5607 |            7,200 |
|  1,000 |    252 |            198.249 |   0.7867 |           12,096 |
| 10,000 |    418 |          2,704.616 |   6.4704 |           20,064 |

10K stress consistency check:

- Stress-lane fairness matrix (`10,000` actors, `418` blocks): `~112.7s` in release, `nonce spread = 3`, starvation-free (`min_nonce = 1`)
- Ringless 10k long-run check (`500` blocks): `~114.3s` in release, `nonce spread <= 3`
- Queue/wakeup occupancy profile (`10,000`, `418`): `max_wakeup_backlog = 10001`, `max_wakeup_buckets = 20`, `max_current_queue_len = 0`

Readiness hot-state impact check (`10,000 actors`, `64 blocks`):

- Comparison profiler runs both modes (`AaaReadiness` hot-state vs fallback-to-instance reconstruction)
- Output includes per-round samples and averaged `ms/block`
- Execution counts are asserted equal across modes to protect semantic equivalence
- Absolute deltas are environment-sensitive and should be treated as diagnostics, not fixed constants

PoV proof-size decomposition benchmark (`scheduler_scan_*`, `n ∈ [100,1000]`):

- Command (hot): `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_scan_hot_readiness --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_scan_hot_readiness.rs`
- Command (fallback): `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_scan_fallback_readiness --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_scan_fallback_readiness.rs`
- At `n=1000` (estimated model):
  - Hot-state path: proof-size `2,628,794`, ref-time `6,241,298,446 ps`
  - Fallback path: proof-size `7,658,794`, ref-time `10,115,500,000 ps`
  - Delta: proof-size `-65.68%`, ref-time `-38.30%`
- Per-actor slope (estimated): proof-size `2537` vs `7567` bytes/read-actor, ref-time `5,986,707` vs `8,900,522` ps/read-actor (hot vs fallback)

Temporal wakeup diagnostics (`scheduler_wakeup_*`, excluded from runtime weight artifact):

- Dense due-drain baseline: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_dense_due_drain --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_dense_due_drain.rs`
- Sparse-gap recovery plateau: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_sparse_gap_recovery --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_sparse_gap_recovery.rs`
- Spillover probe horizon: `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic scheduler_wakeup_spillover_probe --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_scheduler_wakeup_spillover_probe.rs`
- Interpretation: these diagnostics are the evidence gate for any future `tree`/`heap` vocabulary or storage migration; until they show a real win against lifecycle pain points, block-bucketed `WakeupIndex` remains the reference representation.

Close-path complexity diagnostics (`on_close_execution_plan`):

- Dispatch weight anchor: `close_aaa` benchmark now sets up worst-case-like `on_close_execution_plan` (max system steps + max split-transfer legs) before closure
- System diagnostic command (excluded from runtime weight artifact): `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic close_aaa_on_close_execution_plan_complex --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_close_on_close_execution_plan_complex.rs`
- User diagnostic command (excluded from runtime weight artifact): `frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm --pallet pallet_aaa --extrinsic close_aaa_user_fee_bearing_tail --steps 30 --repeat 20 --heap-pages 4096 --output /tmp/pallet_aaa_close_aaa_user_fee_bearing_tail.rs`
- Purpose: expose both non-fee-bearing system close scaling and fee-bearing user close-tail scaling while preserving non-blocking closure semantics
- These close-tail probes remain diagnostic-only in the current line; production weights still come from the normal pallet/runtime benchmarking path rather than being regenerated from this evidence harness
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

Fairness SLO for the current queue topology: high-density matrix and 10K stress runs must stay starvation-free with `nonce spread <= 3` on the documented baseline scenarios.

Local runtime-invariant dry-run wrapper: `./scripts/try-runtime-local.sh --prepare`

---

- `Version`: 0.1.0 (Implementation mirror for [Specification](./aaa.specification.en.md))
- `Last Updated`: March 2026
- `Author`: LLB Lab
- `License`: MIT
