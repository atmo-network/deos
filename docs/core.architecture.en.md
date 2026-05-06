# DEOS Core Architecture: The Token-Driven Economic Automaton

## 1. Executive Summary

The DEOS (Deterministic Economic Operating System) framework, currently instantiated in this repository through the TMCTOL standard, represents a paradigm shift from event-driven blockchain logic to a `Token-Driven Economic Automaton`.

The system operates as a deterministic state machine where specialized actors coordinate exclusively through `Balance Ingress`. It abandons the traditional "Request-Response" model in favor of `Continuous Flow Processing`. The network coordinates through explicit, permissionless token flows between dedicated accounts, ensuring that every state transition is mathematically bounded, economically productive, and immune to intra-block manipulation.

All deterministic economic flows — deflationary burning, liquidity provisioning, treasury management, and protocol-token buyback — are expressed as declarative execution plans on a single runtime platform: `pallet-aaa` (Account Abstraction Actors). Thirteen genesis System actors replace three former standalone pallets, reducing total code by ~6000 LOC while expanding capability to a multi-token protocol economy.

## 2. Core Philosophy: The "Omnivorous" Machine

### 2.1 The Coordination Rule

The entire network follows a single, immutable coordination rule:

> `Balance-in → Deterministic State Transition → Balance-out`

### 2.2 Key Architectural Properties

1. `Origin-Agnostic`: Actors do not validate _who_ sent the tokens. They only validate _what_ arrived. This makes the system permissionless and interoperable by default.
2. `Stateless Execution`: The system minimizes on-chain storage. Intermediate buffers are removed; flows are direct (One-Hop).
3. `Graceful Degradation`: The system is "economically omnivorous." Erroneous transfers (e.g., a user sending funds directly to the Burning Manager sovereign) are not lost errors—they are processed as valid economic contributions (e.g., burnt or added to liquidity).
4. `Reactive Resilience`: The system applies backpressure (cooldown-based retry) instead of failing catastrophically. If conditions are unsafe (e.g., slippage exceeded), actors skip the step and retry on next trigger cycle.
5. `Explicit Read-Model Split`: DEOS separates bounded authoritative on-chain values/projections that clients can consume directly from externally indexed materializations used for archive/search/analytics. Canonical product flows should rely on raw on-chain state when a bounded projection is the real protocol contract; unbounded history and heavy dashboard aggregation should remain off-chain instead of being smuggled into consensus state. The project-wide subsystem matrix and design checklist live in [`read-model.contract.en.md`](./read-model.contract.en.md).

## 3. Actor Architecture & Economic Topology

### 3.1 The Actor Constellation

All actors are System instances managed by `pallet-aaa`. The Axial Router and TMC remain dedicated pallets providing routing and minting infrastructure.

#### L1 Actors (Native Token Domain)

| Actor                   | aaa_id | Execution Plan                                                    | Trigger                      |
| ----------------------- | ------ | ----------------------------------------------------------------- | ---------------------------- |
| Burning Manager         | 0      | `[SwapExactIn(foreign→native)]* → Burn(native)`                   | `Timer { every_blocks: 10 }` |
| Zap Manager             | 2      | `AddLiquidity → SwapExactIn(patriotic) → SplitTransfer(→buckets)` | `Timer { every_blocks: 1 }`  |
| TOL Bucket A (Anchor)   | 3      | `Noop` — permanent LP accumulation                                | `Timer` (Noop)               |
| TOL Bucket B (Building) | 4      | `RemoveLiquidity → Transfer × 2 (→ Treasury B)`                   | `Timer` (future)             |
| TOL Bucket C (Capital)  | 5      | `RemoveLiquidity → Transfer × 2 (→ Treasury C)`                   | `Timer` (future)             |
| TOL Bucket D (Dormant)  | 6      | `Noop` — held for future governance policy                        | `Timer` (Noop)               |
| Treasury B (Building)   | 7      | `SwapExactIn(NTVE→BLDR) → Burn(BLDR)` — buyback & burn            | `Timer { every: 600 }`       |
| Treasury C (Capital)    | 8      | `Noop` — receives Native + Foreign from Bucket C unwind           | `Timer` (Noop)               |
| Treasury D (Dormant)    | 9      | `Noop` — receives Native + Foreign from Bucket D unwind           | `Timer` (Noop)               |

#### L2 Actors (BLDR Protocol Token Domain)

| Actor         | aaa_id | Execution Plan                                                | Trigger                     |
| ------------- | ------ | ------------------------------------------------------------- | --------------------------- |
| BLDR Splitter | 10     | `Transfer(NTVE→ZM) + SplitTransfer(BLDR, 50%→ZM+50%→Trs.)`    | `Timer { every_blocks: 1 }` |
| BLDR ZM       | 11     | `AddLiquidity(NTVE,BLDR) → SplitTransfer(LP → BLDR Bucket A)` | `Timer { every_blocks: 1 }` |
| BLDR Bucket A | 12     | `Noop` — permanent BLDR LP accumulation                       | `Timer` (Noop)              |
| BLDR Treasury | 13     | `Noop` — BLDR ecosystem fund                                  | `Timer` (Noop)              |

See [`aaa.architecture.en.md`](./aaa.architecture.en.md#current-tmctol-system-aaa-topology-on-deos) for the integrated System AAA topology, execution-plan families, and governance activation flows.

### 3.2 Type System Foundation: The Bitmask Architecture

To guarantee O(1) execution complexity and maximal interoperability, the architecture relies on a high-performance `Bitmask Identification Strategy` implemented in `primitives/src/assets.rs`.

#### 3.2.1 Asset Taxonomy

The system uses a 32-bit ID space where the most significant nibble (4 bits) determines the asset category. Five production types are currently defined — additional nibbles are reserved for future use.

| Nibble | Mask          | Constant              | Description                           |
| :----- | :------------ | :-------------------- | :------------------------------------ |
| `0x1`  | `0x1000_0000` | `TYPE_PROTOCOL`       | Protocol-native tokens ($VETO, $BLDR) |
| `0x5`  | `0x5000_0000` | `TYPE_STAKED`         | Native/local staking receipt assets   |
| `0x6`  | `0x6000_0000` | `TYPE_STAKED_FOREIGN` | Foreign staking receipt assets        |
| `0x7`  | `0x7000_0000` | `TYPE_LP`             | Liquidity Pool shares                 |
| `0xF`  | `0xF000_0000` | `TYPE_FOREIGN`        | XCM foreign assets                    |

Nibbles `0x0`, `0x2`–`0x4`, `0x8`–`0xE` are reserved. The `0x0` nibble is intentionally unused — zero type bits cause false positives in `(id & MASK_TYPE) == TYPE` checks.

Native token ($NTVE) uses `AssetKind::Native` enum variant, not a bitmask ID.

#### Protocol Tokens

| Token   | AssetKind            | ID            | Role                                     |
| ------- | -------------------- | ------------- | ---------------------------------------- |
| `$NTVE` | `Native`             | —             | Sovereign currency, L1 TMC emission      |
| `$VETO` | `Local(0x1000_0001)` | `0x1000_0001` | Governance token (deferred)              |
| `$BLDR` | `Local(0x1000_0002)` | `0x1000_0002` | Builder incentive token, L2 TMC emission |

#### 3.2.2 Zero-Cost Abstractions

This architecture enables "Zero-Cost Inspection" where complex economic properties are verified via bitwise operations rather than storage reads.

- `Asset Classification`: Automatically detects asset types (Protocol, LP, Foreign) via bitmask matching for routing decisions.
- `Routing Logic`: The Router uses bitmask inspection to determine optimal paths (e.g., Native-anchored multi-hop for non-Native pairs) without storage lookups.
- `Security`: Namespace isolation prevents LP token ID collisions with Protocol Tokens. On clean-slate genesis, `NextPoolAssetId` is initialized into `TYPE_LP` space so newly minted pool LP IDs are `AssetKind::is_lp()`-detectable by bitmask.

> `Fee Policy`: All asset pairs pay the same flat Router fee (default 0.5%). No discounts or special rates based on asset type. Fee rate is configurable via governance.

### 3.3 Actor Responsibilities

#### 🧠 Axial Router (Pallet — The Decision Engine)

_The intellectual layer atop raw liquidity._

- `Function`: Intelligent Aggregation. Calculates an `Efficiency Score` to choose between:
  - `Market Liquidity`: Standard XYK Swaps.
  - `Protocol Liquidity`: Direct Minting via TMC (if mathematically superior).
  - `Complex Paths`: Multi-hop Native-anchored routes.
- `Multi-Curve Routing`: For any pair where `has_curve(to) && supports_collateral(to, from)`, the Router considers TMC as a candidate route. This generalizes beyond Native-only minting to support BLDR and future protocol tokens.
- `Security Feature`: `Pre-Swap Oracle Update`. The router snapshots pool reserves _before_ execution to update the Oracle. This renders the system immune to Flash Loan attacks.
- `Execution`: Uses `Balance-Delta Verification` (Trustless Execution) — measures the physical change in the recipient's balance rather than relying on theoretical quotes.

#### 📉 TMC (Pallet — The Ceiling)

_The algorithmic issuer._

- `Function`: Unidirectional token emission along a linear price curve. TMC is a `pure minting machine` — it knows nothing about downstream routing, splitting, or liquidity provisioning.
- `Multi-Curve Architecture`: Each curve is identified by its `minted_asset` (formerly `native_asset`). The system supports multiple concurrent curves:
  - L1: `minted_asset = Native`, `foreign_asset = Foreign` (e.g., USDC)
  - L2: `minted_asset = Local(BLDR_ASSET_ID)`, `foreign_asset = Native`
- `MintOutputResolver`: Single-method trait mapping `minted_asset → output_account`. TMC sends both the user's portion directly and routes the zap share (collateral + minted tokens) to a single sink address. The sink is responsible for downstream fan-out.
  - Native → ZapManager sovereign (L1 liquidity provisioning)
  - BLDR → BLDR Splitter sovereign (L2 distribution)
- `Role`: Sets the "Hard Ceiling" on price. If market price > curve price, the Router automatically routes trades through TMC, creating arbitrage that feeds the protocol.

#### 🔥 Burning Manager (System AAA #0 — The Sink)

_The deflationary engine._

- `Function`: Passive accumulation and destruction. Timer-polled every 10 blocks.
- `Execution Plan`: For each registered foreign asset — `SwapExactIn(foreign→native, AllBalance, 5% slippage)` with `BalanceAbove` dust guard. Final step — `Burn(native, AllBalance)`.
- `Resilience`: Swap failures → `ContinueNextStep` → skip to next asset or burn step. Cooldown prevents retry storms.

#### 💸 Fee Sink (System AAA #1 — Unified Fee Collection and Phase 1 Redistribution)

_The launch economics fan-out hub._

- `Function`: Collects all protocol fees and routes them into the active reward flows for the current launch phase.
- `Collection Rule`: Unified 20% collator / 80% Fee Sink split applied to transaction fees, AAA fees, and future block rewards once the block reward source is defined. When the author cannot be resolved, 100% goes to Fee Sink.
- `Phase 1 Execution Plan`: `SplitTransfer(native, AllBalance)` — every block, timer-driven `every_blocks = 1`:
  1. 50% → staking-pool ingress holding account as native balance, later burned and minted into the local native-staking asset after AAA #14 donation execution
  2. 50% → Native Staking LP Farmer AAA #14 as native balance, immediately burned and bridged into the local native-staking asset for donation execution
- `Release Gate`: the Phase 1 staking-yield and LP-donation bridge is wired; block reward source/amount design remains the separate future gate.
- `Phase 2 (future)`: 1∶1∶4 redistribution into staking pool, liquidity pool, and claimable LP-nomination rewards weighted by GovXP.
- `Resilience`: Phase 1 SplitTransfer legs are unwrapped synchronously, with ED preservation for the Fee Sink sovereign account.

#### ⚡ Zap Manager (System AAA #2 — The Transformer)

_The liquidity compositor._

- `Function`: Transforms raw capital into Protocol-Owned Liquidity.
- `Execution Plan` (3 steps, activated by governance after pool creation):
  1. `AddLiquidity` — opportunistic, at current pool ratio (AllBalance native, AllBalance foreign)
  2. `SwapExactIn(foreign→native)` — patriotic accumulation of surplus foreign with reserve-aware slippage derived from current native pool depth
  3. `SplitTransfer(LP → TOL buckets)` — 50/16.67/16.67/16.66% to bucket AAA sovereign accounts
- `Launch Policy`: The current runtime freezes this reserve-aware slippage at execution-plan build time rather than recomputing it live on every cycle; richer live per-cycle recomputation remains a future opt-in refinement, not launch debt.
- `Genesis`: Noop skeleton. Real execution plan installed after pools exist.

#### 🏛️ TOL Buckets (System AAA #3..#6 — The Floor)

_The volatility dampener._

- `Function`: Four independent actors holding protocol-owned LP tokens. Each bucket is a sovereign entity with its own lifecycle.
- `Genesis`: All Noop. Governance activates bucket-specific policies via `update_execution_plan`.
- `Buckets`:
  - _Bucket A — Anchor (50%):_ Permanent LP accumulation. No unwind — the mathematical survival layer.
  - _Bucket B — Building (16.67%):_ Gradual LP unwind → Native + Foreign → Treasury B (BLDR buyback).
  - _Bucket C — Capital (16.67%):_ Gradual LP unwind → Native + Foreign → Treasury C (operational).
  - _Bucket D — Dormant (16.66%):_ LP held until governance decides future policy.

#### 💰 Treasury B — BLDR Buyback & Burn (System AAA #7)

_The deflationary flywheel for protocol tokens._

- `Function`: Accumulates NTVE from Bucket B unwind. Uses ~1%/day of current balance to buy BLDR on the open market and burn it.
- `Execution Plan` (2 steps, activated by governance):
  1. `SwapExactIn(NTVE → BLDR, PercentageOfCurrent(~0.042%))` — hourly micro-buyback
  2. `Burn(BLDR, AllBalance)` — destroy all acquired BLDR
- `Cadence`: `Timer { every_blocks: 600 }` (~1 hour). Compounding: `(1-0.000418)^24 ≈ 0.99` → ~1% of balance/day.
- `Design`: Multiple small buybacks create smooth market pressure vs. single lumpy daily purchase.

#### 🔀 BLDR Splitter (System AAA #10 — L2 Distribution Hub)

_The fan-out actor for BLDR TMC output._

- `Function`: Receives both NTVE collateral and BLDR zap share from TMC MintOutputResolver. Distributes to BLDR ZM and BLDR Treasury.
- `Execution Plan` (2 steps):
  1. `Transfer(NTVE, AllBalance → BLDR ZM)` — collateral for liquidity provisioning
  2. `SplitTransfer(BLDR, AllBalance, 50% → BLDR ZM, 50% → BLDR Treasury)` — token distribution

#### ⚡ BLDR Zap Manager (System AAA #11 — L2 Liquidity)

_The BLDR-domain liquidity provisioner._

- `Function`: Provisions NTVE-BLDR LP and routes LP tokens to BLDR Bucket A.
- `Execution Plan` (2 steps):
  1. `AddLiquidity(NTVE, BLDR)` — opportunistic at current pool ratio
  2. `SplitTransfer(LP, AllBalance → BLDR Bucket A)` — 100% to permanent accumulation

### 3.4 Token Lifecycle Orchestration

Token onboarding follows a governance-gated process:

**Foreign asset onboarding (L1):**

1. `Asset Registration`: Register foreign asset in `pallet-asset-registry` (`Location → AssetId`).
2. `Pool Creation`: Create AMM pool via `pallet-asset-conversion`.
3. `BM Execution-Plan Extension`: Call `update_execution_plan` on AAA #0 to add `SwapExactIn(foreign→native)` step.
4. `ZM Execution-Plan Activation`: Call `update_execution_plan` on AAA #2 with `build_zap_execution_plan(foreign, lp_asset, dust)`.
5. `TMC Curve Activation` (optional): Create emission curve for the token.

**Protocol token onboarding (L2 — BLDR pattern):**

1. `Asset Creation`: Create protocol token in `pallet-assets`.
2. `TMC Curve Creation`: `create_curve(BLDR, Native, initial_price, slope)`.
3. `Pool Creation`: Create NTVE-BLDR AMM pool + seed initial liquidity.
4. `Execution-Plan Activation`: Activate BLDR Splitter and BLDR ZM execution plans via governance.

Each step is independently reversible and governance-gated. No implicit cross-pallet coupling — each execution-plan update is explicit and auditable.

### 3.5 Antifragile Design Principles

- `Fail-fast over silent drift`: AAA step errors are handled by explicit `on_error` policy (`ContinueNextStep` / `AbortCycle`). No silent state corruption.
- `Idempotent operations`: Execution-plan steps are stateless — each execution starts from current on-chain balances, not accumulated state.
- `Observable automation`: Every cycle emits `CycleStarted` / `StepFailed` / `CycleSummary` events. Silent stalls are detectable via `IdleStarvationBlocks` counter.
- `Bounded execution`: AAA scheduler uses budget-cap admission with `cycle_weight_upper_bound`. No unbounded loops.

## 4. Deterministic Execution via AAA Scheduler

### 4.1 The Unified Execution Model

All deterministic economic flows execute through a single `pallet-aaa` scheduler, replacing the former per-pallet `on_initialize` / `on_idle` hooks.

#### Execution Architecture

```
Block N:
  on_initialize:
    - Emergency path: if IdleStarvationBlocks > threshold, execute one System AAA
    - Inbox processing: drain AddressEvent inbox for OnAddressEvent-triggered actors

  on_idle(remaining_weight):
    - refill_ready_ring: scan actors, evaluate triggers, enqueue ready ones
    - execute_cycle(budget):
        - Weighted fairness: alternate System/User actors
        - For each ready actor:
            1. Budget admission (weight + fee pre-flight)
            2. Execution-plan execution (step by step)
            3. Condition evaluation per step
            4. Task execution via adapter (DexOps / AssetOps)
            5. Error handling per on_error policy
    - execute_zombie_sweep: clean up closed actors
```

#### Key Properties

- `Budget-capped`: Scheduler consumes the full remaining `on_idle` weight after bounded housekeeping in the hook.
- `Fair scheduling`: System and User actors alternate based on configurable fairness weights. No single actor class can starve the other.
- `Starvation observability`: If `on_idle` receives zero weight for consecutive blocks (`IdleStarvationBlocks`), runtime emits `IdleStarvationDetected`; recovery is governance-operated.
- `Deferred requeue`: Actors that can't execute because of insufficient weight are re-enqueued; User fee insufficiency is terminal (`FeeBudgetExhausted`).

### 4.2 Resilience: Backpressure via Cooldown

The system implements "Economic Backpressure" to handle volatility gracefully.

- `Problem`: If DEX conditions are unfavorable (high slippage, low liquidity), executing a swap is dangerous.
- `Solution`: Step failure → `ContinueNextStep` → execution plan degrades gracefully. Cooldown prevents immediate retry. Actor re-enters the scheduler after the next eligible cooldown boundary.
- `Result`: The system "waits out" the volatility or attack, resuming only when conditions stabilize.

### 4.3 Adapter Architecture

AAA delegates all external operations to two adapter traits:

| Adapter    | Responsibility                                                         | Runtime Implementation                    |
| ---------- | ---------------------------------------------------------------------- | ----------------------------------------- |
| `DexOps`   | `swap_exact_in`, `swap_exact_out`, `add_liquidity`, `remove_liquidity` | Routes through Axial Router               |
| `AssetOps` | `balance`, `transfer`, `burn`, `mint`, `total_issuance`                | Wraps `pallet-balances` + `pallet-assets` |

Adapters are pure pass-through — they execute operations without reinterpreting execution-plan parameters. `swap_exact_in` computes `min_out` internally via DEX quote, while `swap_exact_out` deterministically resolves required input before execution.

### 4.4 Amount Resolution

Execution-plan steps specify amounts via `AmountResolution`, enabling both static and dynamic resolution:

| Variant                            | Description                                   | Use Case                           |
| ---------------------------------- | --------------------------------------------- | ---------------------------------- |
| `Fixed(Balance)`                   | Absolute amount                               | Known-quantity transfers           |
| `PercentageOfCurrent(Perbill)`     | % of actor's current spendable balance        | Gradual unwind (1%/day)            |
| `PercentageOfTrigger(Perbill)`     | % of balance at trigger evaluation time       | Event-proportional actions         |
| `PercentageOfLastFunding(Perbill)` | % of last funding snapshot                    | DCA-style repeated purchases       |
| `AllBalance`                       | Entire spendable balance (ED-safe for native) | Drain steps (burn, final transfer) |

## 5. Code Integration Patterns

### 5.1 Trustless Execution Pattern

The Router does not trust the return value of the AMM. It verifies the physical reality of the ledger using type-safe inspection.

```rust
// AssetConversionAdapter
fn swap_exact_tokens_for_tokens(...) -> Result<Balance, DispatchError> {
    let balance_before = match target_asset {
        AssetKind::Native => T::Currency::balance(&recipient),
        AssetKind::Local(id) => T::Assets::balance(id, &recipient),
    };
    AssetConversion::swap_exact_tokens_for_tokens(...)?;
    let balance_after = match target_asset {
        AssetKind::Native => T::Currency::balance(&recipient),
        AssetKind::Local(id) => T::Assets::balance(id, &recipient),
    };
    Ok(balance_after.saturating_sub(balance_before))
}
```

### 5.2 Flash-Loan Resistant Oracle Pattern

The system updates the pricing model based on the state _before_ the transaction distorts it.

```rust
pub fn swap(from: AssetKind, to: AssetKind, ...) -> DispatchResult {
    // Update Oracle using Pre-Swap Reserves (immune to flash loans)
    Self::update_oracle_from_reserves(from, to)?;
    Self::execute_optimal_route(...)?;
    Ok(())
}
```

### 5.3 Declarative Execution-Plan Pattern

All deterministic economic logic is expressed as composable execution plans:

```rust
// Burning Manager execution plan: swap all foreign → native, then burn
vec![
    Step { conditions: [BalanceAbove(foreign, dust)],
           task: SwapExactIn { foreign → Native, AllBalance, 5% slippage },
           on_error: ContinueNextStep },
    Step { conditions: [BalanceAbove(Native, dust)],
           task: Burn { Native, AllBalance },
           on_error: AbortCycle },
]

// BLDR Splitter execution plan: forward NTVE, split BLDR
vec![
    Step { conditions: [BalanceAbove(Native, dust)],
           task: Transfer { Native, AllBalance → BLDR ZM },
           on_error: ContinueNextStep },
    Step { conditions: [BalanceAbove(BLDR, dust)],
           task: SplitTransfer { BLDR, AllBalance, 50%→ZM + 50%→Treasury },
           on_error: AbortCycle },
]

// Treasury B buyback: buy BLDR with 0.042% of current NTVE balance, burn
vec![
    Step { conditions: [BalanceAbove(Native, dust)],
           task: SwapExactIn { Native → BLDR, PercentageOfCurrent(0.042%), 5% },
           on_error: AbortCycle },
    Step { conditions: [BalanceAbove(BLDR, dust)],
           task: Burn { BLDR, AllBalance },
           on_error: AbortCycle },
]
```

### 5.4 Unified Type System Pattern

Centralizing type definitions to break dependency cycles.

```rust
// All pallets and runtime use the same types from primitives crate
pub use primitives::AssetKind;         // Bitmask-based asset classification
pub use primitives::ecosystem;         // Constants, pallet IDs, AAA IDs
pub use primitives::protocol_tokens;   // VETO_ASSET_ID, BLDR_ASSET_ID
```

## 6. Network Architecture: The Connected Automaton

The DEOS reference runtime extends its "Omnivorous" philosophy to the Polkadot ecosystem via XCM (Cross-Consensus Messaging), treating foreign chains as just another source of balance ingress.

### 6.1 XCM Integration Strategy

The parachain acts as a `Sovereign Liquidity Hub`, accepting assets from Relay Chain and Sibling Parachains after governance registration in the Asset Registry.

- `Ingress Protocol`: The system accepts `ReserveAssetDeposited` and `Teleport` instructions.
- `Asset Mapping (Hybrid)`: bidirectional `Location <-> AssetId` stored on-chain in the Asset Registry; IDs are generated once at registration (`hash(Location)`) and then persisted as the stable identity contract. This protects against XCM location-key drift while keeping forward lookup, reverse lookup, and bijectivity O(1).
- `Holding Register`: Incoming assets are held in a temporary register before being dispatched to the `ForeignAssetsTransactor`.
- `Sovereign Transact Surface`: The current reference line keeps barrier/origin-conversion plumbing for paid and explicit unpaid execution classes, but exposes no sovereign-XCM runtime-call dispatch surface by default; `SafeCallFilter = Nothing` makes `Transact` fail-closed unless a later constitutional/runtime slice explicitly opts concrete calls in.

### 6.2 Foreign Asset Transactor

The `ForeignAssetsTransactor` (configured in `xcm_config.rs`) provides the bridge between XCM locations and the internal `pallet-assets` registry.

- `Storage Lookup`: Uses the Asset Registry mapping (O(1) storage) to resolve `Location -> AssetId` (`0xF...` namespace), with symmetric reverse lookup available when runtime integration needs `AssetId -> Location`.
- `Governance-Gated Onboarding`: New assets are registered via registry extrinsics (deterministic ID, manual ID, or linking pre-created `0xF...`), then consumed by XCM flows.

### 6.3 Cross-Chain Identity

- `Sovereignty`: The parachain maintains sovereign accounts on other chains to manage its own liquidity reserves.
- `Sibling Recognition`: `ForeignAssetsFromSibling` filter ensures that assets originating from sibling parachains are recognized as valid reserve assets, enabling seamless cross-chain swaps.
- `Controller Separation`: XCMP queue control remains Root-only on the current line even though relay/sibling/account-style XCM origins can still be converted for other executor/controller plumbing; origin conversion does not itself widen queue-control authority.

## 7. Economic Guarantees

### 7.1 The Price Corridor

The interaction of actors creates a mathematically bounded economy:

- `Ceiling`: Enforced by TMC (Infinite supply at Curve Price). Multiple curves (Native, BLDR) each define independent ceilings.
- `Floor`: Enforced by TOL buckets (Deep Protocol-Owned Liquidity via AAA #3..#6 for L1, AAA #12 for L2 BLDR).
- `Compression`: BM burns reduce `TotalIssuance` (lowering ceiling). ZM LP provisioning into TOL raises the floor. Treasury B BLDR buyback-burn creates deflationary pressure on BLDR. Together they create **bidirectional compression** — the Gravity Well.

### 7.2 Deflationary Velocity

The `Axial Router` acts as a vacuum for circulating supply.

- `Mechanism`: High base fee (e.g., 0.5%) + Protocol Priority Routing.
- `Outcome`: System value capture is prioritized over LP revenue. The protocol captures the spread to burn its own supply.

### 7.3 Multi-Token Flywheel

The BLDR economy creates a self-reinforcing loop:

```
User buys BLDR (via Router TMC)
  → NTVE collateral → BLDR Splitter → BLDR ZM → LP → BLDR Bucket A (floor ↑)
  → BLDR zap share → 50% to ZM (more LP), 50% to Treasury (ecosystem fund)
  → Bucket B unwind → Treasury B → buyback BLDR on XYK → burn (supply ↓)
```

Both BLDR floor (LP accumulation) and ceiling (supply burn) compress over time.

## 8. Runtime Topology

### 8.1 Pallet Inventory

| Pallet                    | Role                                                     | Hooks                                                |
| ------------------------- | -------------------------------------------------------- | ---------------------------------------------------- |
| `pallet-aaa`              | Deterministic actor platform (13 System + N User actors) | `on_initialize` (bookkeeping), `on_idle` (scheduler) |
| `pallet-axial-router`     | Trade routing, fee collection, oracle                    | Extrinsic-driven                                     |
| `pallet-tmc`              | Unidirectional token emission (multi-curve)              | Extrinsic-driven                                     |
| `pallet-asset-registry`   | Foreign asset registration, Location→AssetId mapping     | Extrinsic-driven                                     |
| `pallet-asset-conversion` | Uniswap V2-like AMM pools                                | Extrinsic-driven                                     |
| `pallet-assets`           | Fungible asset ledger                                    | —                                                    |
| `pallet-balances`         | Native token ledger                                      | —                                                    |

### 8.2 Former Pallets (Consolidated into AAA)

| Former Pallet                     | Replacement                         | LOC Saved |
| --------------------------------- | ----------------------------------- | --------- |
| `pallet-burning-manager`          | System AAA #0 (BM execution plan)   | ~1100     |
| `pallet-zap-manager`              | System AAA #2 (ZM execution plan)   | ~2200     |
| `pallet-treasury-owned-liquidity` | System AAA #3..#6 (4 bucket actors) | ~3100     |

Additional cleanup: `TolZapInterface` trait and `TolZapAdapter` removed from TMC (~150 LOC), superseded by `MintOutputResolver` + AAA execution-plan-driven routing.

## 9. Conclusion

The DEOS architecture transforms the blockchain from a passive ledger into an `Active Economic Automaton`.

By expressing all deterministic economic flows as declarative execution plans on a single platform (`pallet-aaa`), the system achieves:

1. `Maximum Security`: Immune to Flash Loans and Dust Attacks. Slippage protection in every swap.
2. `Composable Automation`: bounded AAA `Task` primitives compose into any economic flow. New actors require zero code changes.
3. `Multi-Token Economy`: L1 (Native) and L2 (BLDR) economies operate independently with shared infrastructure. Each has its own TMC curve, liquidity pools, ZM, and TOL buckets.
4. `Optimal Performance`: AAA scheduler uses budget-capped `on_idle` with bounded `on_initialize` bookkeeping only. No block bloat.
5. `Total Autonomy`: The economy runs itself — burning fees, provisioning liquidity, buying back protocol tokens, and managing treasury buckets every block cycle.
6. `Radical Simplicity`: 4 pallets replace 7. Net −6000 LOC. Every deleted line is a line that can't have bugs.

---

- `Version`: 0.1.0
- `Last Updated`: March 2026
