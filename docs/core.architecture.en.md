# DEOS Core Architecture: The Token-Driven Economic Automaton

## 1. Executive Summary

The DEOS (Deterministic Economic Operating System) framework, currently instantiated in this repository through the TMCTOL standard, combines dedicated economic pallets with a bounded actor scheduler. Token movement can act as an explicit coordination message rather than requiring one bespoke privileged call per flow.

The runtime remains a deterministic state machine. AAA actors become eligible through typed balance ingress, timers, or manual governance/owner signals, then execute only fully admitted work through runtime adapters. Route-specific validation mitigates some manipulation paths, but it does not provide blanket immunity to intra-block ordering, MEV, flash-loan, or sandwich risks.

Recurring protocol automation that current bounded tasks/adapters can express — including burning, liquidity provisioning, treasury routing, and protocol-token buyback patterns — uses declarative `pallet-aaa` execution plans. Fifteen genesis System actors consolidate the reference topology, while TMC, routing, staking, balances, and AMM mechanics remain owned by their dedicated pallets. A flow that cannot preserve custody, atomicity, or production-budget admission through existing primitives does not ship merely because its vector shape is bounded.

## 2. Core Philosophy: The "Omnivorous" Machine

### 2.1 The Coordination Rule

Token-driven actor flows follow this bounded coordination pattern:

> `Authorized balance ingress → Admitted deterministic transition → Observable balance/state result`

### 2.2 Key Architectural Properties

1. `Provenance-Aware Ingress`: Source/asset trigger filters and funding-source policy decide whether a deposit influences readiness or funding snapshots; ordinary balance credit remains separate from execution authority.
2. `Plan-Local Statelessness`: Steps read current state without mutable cross-step scratch storage, while bounded lifecycle, readiness, queue, funding-batch, and observability state remains explicit on-chain.
3. `Donation Sensitivity`: Assets transferred to a sovereign account remain real balances, but only configured tasks and authorized trigger/funding semantics determine whether and when they affect protocol execution. A donation is not automatically a burn or liquidity contribution.
4. `Reactive Resilience`: Explicit `StepErrorPolicy`, cooldowns, paged FIFO readiness, and exact temporal wakeups provide bounded backpressure. Unsafe conditions may skip or abort a cycle; subsequent execution still requires a valid trigger and sufficient two-dimensional budget.
5. `Explicit Read-Model Split`: DEOS separates bounded authoritative on-chain values/projections that clients can consume directly from externally indexed materializations used for archive/search/analytics. Canonical product flows should rely on raw on-chain state when a bounded projection is the real protocol contract; unbounded history and heavy dashboard aggregation should remain off-chain instead of being smuggled into consensus state. The project-wide subsystem matrix and design checklist live in [`read-model.contract.en.md`](./read-model.contract.en.md).

## 3. Actor Architecture & Economic Topology

### 3.1 The Actor Constellation

The reference constellation below uses System AAA instances; the pallet also supports bounded owner-controlled User actors. The Axial Router and TMC remain dedicated pallets providing routing and minting infrastructure.

#### L1 Actors (Native Token Domain)

| Role | aaa_id | Genesis state | Program/trigger |
| --- | --- | --- | --- |
| Burn Actor | 0 | Active System AAA | Burn plan; omnivorous `OnAddressEvent` |
| Fee Sink | 1 | Active System AAA | Phase-one fee allocation; omnivorous `OnAddressEvent` |
| Liquidity Actor | 2 | Dormant System identity | No program until pool-specific activation |
| TOL Bucket A (Anchor) | 3 | Custody-only account | No generic AAA identity or program |
| TOL Bucket B (Building) | 4 | Dormant System identity | No program until explicit activation |
| TOL Bucket C (Capital) | 5 | Dormant System identity | No program until explicit activation |
| TOL Bucket D (Dormant) | 6 | Dormant System identity | No program until explicit activation |
| Treasury B (Building) | 7 | Dormant System identity | No program until explicit activation |
| Treasury C (Capital) | 8 | Dormant System identity | No program until explicit activation |
| Treasury D (Dormant) | 9 | Dormant System identity | No program until explicit activation |
| Native Staking LP Farmer | 14 | Dormant System identity | No program until pool-specific activation |

#### L2 Actors (BLDR Protocol Token Domain)

| Role | aaa_id | Genesis state | Program/trigger |
| --- | --- | --- | --- |
| BLDR Splitter | 10 | Active System AAA | Split BLDR 50/50; omnivorous `OnAddressEvent` |
| BLDR Liquidity Actor | 11 | Dormant System identity | No program until NTVE/BLDR pool activation |
| BLDR Bucket A | 12 | Custody-only account | No generic AAA identity or program |
| BLDR Treasury | 13 | Dormant System identity | No program until explicit activation |

See [`aaa.architecture.en.md`](./aaa.architecture.en.md#current-tmctol-system-aaa-topology-on-deos) for the integrated System AAA topology, execution-plan families, and governance activation flows.

### 3.2 Type System Foundation: The Bitmask Architecture

To guarantee O(1) execution complexity and maximal interoperability, the architecture relies on a high-performance `Bitmask Identification Strategy` implemented in `primitives/src/assets.rs`.

#### 3.2.1 Asset Taxonomy

The system uses a 32-bit ID space where the most significant nibble (4 bits) determines the asset category. Five production types are currently defined — additional nibbles are reserved for future use.

| Nibble | Mask | Constant | Description |
| --- | --- | --- | --- |
| `0x1` | `0x1000_0000` | `TYPE_PROTOCOL` | Protocol-native tokens ($VETO, $BLDR) |
| `0x5` | `0x5000_0000` | `TYPE_STAKED` | Native/local staking receipt assets |
| `0x6` | `0x6000_0000` | `TYPE_STAKED_FOREIGN` | Foreign staking receipt assets |
| `0x7` | `0x7000_0000` | `TYPE_LP` | Liquidity Pool shares |
| `0xF` | `0xF000_0000` | `TYPE_FOREIGN` | XCM foreign assets |

Nibbles `0x0`, `0x2`–`0x4`, `0x8`–`0xE` are reserved. The `0x0` nibble is intentionally unused — zero type bits cause false positives in `(id & MASK_TYPE) == TYPE` checks.

Native token ($NTVE) uses `AssetKind::Native` enum variant, not a bitmask ID.

#### Protocol Tokens

| Token | AssetKind | ID | Role |
| --- | --- | --- | --- |
| `$NTVE` | `Native` | — | Sovereign currency, L1 TMC emission |
| `$VETO` | `Local(0x1000_0001)` | `0x1000_0001` | Governance token (deferred) |
| `$BLDR` | `Local(0x1000_0002)` | `0x1000_0002` | Builder incentive token, L2 TMC emission |

#### 3.2.2 Zero-Cost Abstractions

This architecture enables "Zero-Cost Inspection" where complex economic properties are verified via bitwise operations rather than storage reads.

- `Asset Classification`: Automatically detects asset types (Protocol, LP, Foreign) via bitmask matching for routing decisions.
- `Routing Logic`: The Router uses bitmask inspection to determine optimal paths (e.g., Native-anchored multi-hop for non-Native pairs) without storage lookups.
- `Security`: Namespace isolation prevents LP token ID collisions with Protocol Tokens. On clean-slate genesis, `NextPoolAssetId` is initialized into `TYPE_LP` space so newly minted pool LP IDs are `AssetKind::is_lp()`-detectable by bitmask.

> `Fee Policy`: All asset pairs pay the same flat Router fee (default 0.5%). No discounts or special rates based on asset type. Fee rate is configurable via governance.

### 3.3 Actor Responsibilities

#### Axial Router (Pallet — The Decision Engine)

_The intellectual layer atop raw liquidity._

- `Function`: Mechanism-only aggregation. Among candidate routes it always selects **max recipient output** (`max_by_key` on `expected_output`), not a policy score:
  - `Market Liquidity`: Direct XYK swaps.
  - `Protocol Liquidity`: Direct minting via TMC when that path delivers more output.
  - `Complex Paths`: Multi-hop Native-anchored XYK routes.
- `Multi-Curve Routing`: For any pair where `has_curve(to) && supports_collateral(to, from)`, the Router considers TMC as a candidate route. This generalizes beyond Native-only minting to support BLDR and future protocol tokens.
- `Security mitigations (not complete MEV defense)`: **Direct** routes validate the candidate quote against the previously stored EMA, then snapshot current pre-execution pool reserves into the EMA before executing the swap; multi-hop routes rely on user `min_amount_out` / slippage only. This launch line has no commit/reveal or ordering protection — do not claim flash-loan or sandwich immunity.
- `Execution`: Uses `Balance-Delta Verification` (Trustless Execution) — measures the physical change in the recipient's balance rather than relying on theoretical quotes.

#### TMC (Pallet — The Ceiling)

_The algorithmic issuer._

- `Function`: Unidirectional token emission along a linear price curve. TMC is a `pure minting machine` — it knows nothing about downstream routing, splitting, or liquidity provisioning.
- `Multi-Curve Architecture`: Each curve is identified by its `minted_asset` (formerly `native_asset`). The system supports multiple concurrent curves:
  - L1: `minted_asset = Native`, `foreign_asset = Foreign` (e.g., USDC)
  - L2: `minted_asset = Local(BLDR_ASSET_ID)`, `foreign_asset = Native`
- `MintOutputResolver`: Maps each `minted_asset` to explicit collateral and minted-liquidity output accounts. The reference L1 route sends both outputs to the Liquidity Actor; the BLDR route sends NTVE collateral directly to the BLDR Liquidity Actor and the minted BLDR share to the BLDR Splitter.
- `Role`: Sets the "Hard Ceiling" on price. If market price > curve price, the Router automatically routes trades through TMC, creating arbitrage that feeds the protocol.

#### Burn Actor (System AAA #0 — The Sink)

_The deflationary engine._

- `Function`: Passive accumulation and destruction. Omnivorous `OnAddressEvent` intake schedules one bounded pass after matched inbound value, subject to the configured cooldown.
- `Execution Plan`: For each registered foreign asset — `SwapExactIn(foreign→native, AllBalance, 5% slippage)` with `BalanceAbove` dust guard. Final step — `Burn(native, AllBalance)`.
- `Resilience`: Swap failures → `ContinueNextStep` → skip to next asset or burn step. Cooldown prevents retry storms.

#### Fee Sink (System AAA #1 — Unified Fee Collection and Phase 1 Redistribution)

_The launch economics fan-out hub._

- `Function`: Collects all non-Axial protocol action fees and routes them into the active reward flows for the current launch phase; Axial Router trading fees remain the dedicated Burn Actor flow.
- `Collection Rule`: 100% of transaction, AAA, governance-opening, and XCM-execution fees enters Fee Sink before allocation; collection never pays the current author directly.
- `Phase 1 Execution Plan`: `SplitTransfer(native, AllBalance)` — omnivorous `OnAddressEvent` intake schedules the fan-out after matched inbound value, subject to the configured cooldown:
  1. 50% → staking-pool ingress holding account as native balance, later burned and minted into the local native-staking asset after AAA #14 donation execution
  2. 50% → native staking LP provisioning actor AAA #14 as native balance, immediately burned and bridged into the local native-staking asset for donation execution
- `Release Gate`: the Phase 1 staking-yield and LP-donation bridge is wired; block reward source/amount design remains the separate future gate.
- `Permissionless Phase (future)`: Equal thirds flow to bounded security rewards, staking ingress, and liquidity provisioning only after permissionless collators and the security-reward settlement contract ship; indivisible remainder stays in Fee Sink for a later cycle, and no `CollatorRewardPot` topology is assumed before that gate.
- `Resilience`: Phase 1 SplitTransfer legs are unwrapped synchronously, with ED preservation for the Fee Sink sovereign account.

#### Liquidity Actor (System AAA #2 — The Transformer)

_The liquidity compositor._

- `Function`: Transforms raw capital into Protocol-Owned Liquidity.
- `Execution Plan` (3 steps, activated by governance after pool creation):
  1. `AddLiquidity` — opportunistic, at current pool ratio (AllBalance native, AllBalance foreign)
  2. `SwapExactIn(foreign→native)` — patriotic accumulation of surplus foreign with reserve-aware slippage derived from current native pool depth
  3. `SplitTransfer(LP → TOL buckets)` — 50/16.67/16.67/16.66% to bucket AAA sovereign accounts
- `Launch Policy`: The current runtime freezes this reserve-aware slippage at execution-plan build time rather than recomputing it live on every cycle; richer live per-cycle recomputation remains a future opt-in refinement, not launch debt.
- `Genesis`: Dormant System identity with no program. Activation installs the real execution plan after pools exist.

#### TOL Buckets (System AAA #3..#6 — The Floor)

_The volatility dampener._

- `Function`: Four sovereign accounts separate protocol-owned LP custody and optional policy lanes. Bucket A remains custody-only; B/C/D retain dormant System identities with independently activatable lifecycles.
- `Genesis`: Bucket A owns no generic AAA program. B/C/D own dormant identities and enroll no scheduler work until activation.
- `Buckets`:
  - _Bucket A — Anchor (50%):_ Permanent LP accumulation. No unwind — the mathematical survival layer.
  - _Bucket B — Building (16.67%):_ Dormant identity; governance may activate bounded LP transfer into Treasury B, whose separate admitted cycle removes liquidity.
  - _Bucket C — Capital (16.67%):_ Dormant identity; governance may activate bounded LP transfer into Treasury C, whose separate admitted cycle removes liquidity.
  - _Bucket D — Dormant (16.66%):_ LP held until governance decides future policy.

#### Treasury B — BLDR Buyback & Burn (System AAA #7)

_The deflationary flywheel for protocol tokens._

- `Function`: Receives LP through the optional admitted Bucket B transfer and can remove it into Treasury custody; the downstream buyback plan remains a separate optional policy stage.
- `Execution Plan` (2-step policy target, not currently production-admissible):
  1. `SwapExactIn(NTVE → BLDR, PercentageOfCurrent(~0.042%))` — hourly micro-buyback
  2. `Burn(BLDR, AllBalance)` — destroy all acquired BLDR
- `Cadence`: `Timer { every_blocks: 600 }` (~1 hour). Compounding: `(1-0.000418)^24 ≈ 0.99` → ~1% of balance/day.
- `Design`: Multiple small buybacks create smooth market pressure vs. single lumpy daily purchase.

#### BLDR Splitter (System AAA #10 — L2 Distribution Hub)

_The fan-out actor for BLDR TMC output._

- `Function`: Receives the minted BLDR liquidity share from TMC and distributes it between the BLDR Liquidity Actor and BLDR Treasury; TMC routes NTVE collateral directly to the Liquidity Actor.
- `Execution Plan`: `SplitTransfer(BLDR, AllBalance, 50% → BLDR Liquidity Actor, 50% → BLDR Treasury)`.

#### BLDR Liquidity Actor (System AAA #11 — L2 Liquidity)

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
3. `Burn Actor Execution-Plan Extension`: Call `update_execution_plan` on AAA #0 to add `SwapExactIn(foreign→native)` step.
4. `Liquidity Actor Execution-Plan Activation`: Call `update_execution_plan` on AAA #2 with `build_zap_execution_plan(foreign, lp_asset, dust)`.
5. `TMC Curve Activation` (optional): Create emission curve for the token.

**Protocol token onboarding (L2 — BLDR pattern):**

1. `Asset Creation`: Create protocol token in `pallet-assets`.
2. `TMC Curve Creation`: `create_curve(BLDR, Native, initial_price, slope)`.
3. `Pool Creation`: Create NTVE-BLDR AMM pool + seed initial liquidity.
4. `Execution-Plan Activation`: Activate BLDR Splitter and BLDR Liquidity Actor execution plans via governance.

Each step is independently reversible and governance-gated. No implicit cross-pallet coupling — each execution-plan update is explicit and auditable.

### 3.5 Antifragile Design Principles

- `Fail-fast over silent drift`: AAA step errors use explicit `ContinueNextStep`, `AbortCycle`, or Mutable-only `RetryLater`; only adapter-classified Temporary failure may suspend. No silent retry heuristic exists.
- `Static operations`: Tasks own no mutable workflow memory. Sparse scheduler-owned Continuation records only bounded unresolved-suffix progress while a Mutable run remains suspended.
- `Observable automation`: One `CycleStarted` and terminal `CycleSummary` bound each logical run; continuation events correlate attempts. Sparse starvation transitions expose scheduler-budget phase changes.
- `Bounded execution`: AAA admits opening plans or unresolved retry suffixes plus terminal cleanup under both Weight dimensions. No unbounded loops exist.

## 4. Deterministic Execution via AAA Scheduler

### 4.1 The Unified Execution Model

AAA-managed recurring automation executes through one `pallet-aaa` scheduler; task adapters call the dedicated pallets that own minting, routing, staking, balances, and AMM state.

#### Execution Architecture

```
Block N:
  on_initialize:
    - Bounded bookkeeping only; never execute AAA cycles

  on_idle(remaining_weight):
    - Admit the generated fixed hook base in both Weight dimensions
    - Drain one saturated-queue tombstone unit when required
    - Run bounded zombie-sweep housekeeping
    - Drain exact due wakeups, snapshot the paged-FIFO tail cutoff, and scan from QueueHead
    - Process actors in deterministic FIFO order up to MaxExecutionsPerBlock:
        1. Reserve the complete actor-probe bound
        2. Apply trigger, cooldown, breaker, window, fee, and lifecycle gates
        3. Admit the complete opening plan or unresolved Continuation suffix plus pure cleanup
        4. Execute the bounded attempt through runtime adapters
        5. Leave weight-deferred head work in place and append late work beyond the cutoff
```

#### Key Properties

- `Budget-capped`: Every housekeeping, queue, cycle, and close unit starts only after two-dimensional admission against the remaining `on_idle` budget.
- `Deterministic FIFO fairness`: Monotonic tickets preserve bounded FIFO carry-over without class-weight knobs or queue reconstruction; measured stress profiles, rather than a System/User alternation claim, own the current fairness SLO.
- `Starvation observability`: After the fixed hook base is admitted, exhaustion of either post-housekeeping Weight dimension transitions sparse `IdleStarvationState`; detection and recovery remain observability-only and never dispatch emergency work in `on_initialize`.
- `Deferred requeue`: Actors that cannot execute because of insufficient weight retain a bounded path to future eligibility; User fee insufficiency remains terminal (`FeeBudgetExhausted`).

### 4.2 Resilience: Backpressure via Cooldown

The system implements "Economic Backpressure" to handle volatility gracefully.

- `Problem`: If DEX conditions are unfavorable (high slippage, low liquidity), executing a swap is dangerous.
- `Solution`: A final failure may use `ContinueNextStep` to advance, while a Mutable plan may use `RetryLater` only for an adapter-classified Temporary failure. Cooldown schedules the unresolved suffix through the canonical FIFO/wakeup substrate.
- `Result`: Temporary incapacity can preserve a committed prefix and resume at the same step. Permanent and unsupported failures never create Continuation.

### 4.3 Adapter Architecture

AAA delegates host behavior through typed runtime contracts:

- `AssetOps`: transferable balances, transfer, burn, mint, minimum-balance, and deposit checks
- `DexOps`: caller-aware swaps plus liquidity addition and removal
- `StakingOps`: runtime-defined stake positions, shares, and transferable share assets
- `LiquidityDonationOps`: adapter-owned pair balancing and receipt-suppression semantics
- `FeeCollector`, `FundingAuthority`, direct ingress, task-failure retry class, task weights, and atomicity hooks: host-owned authority, metering, provenance, and lifecycle integration

Adapters own runtime-specific mechanics while AAA owns plan resolution, admission, task-scoped rollback, and observable error-policy handling. The detailed portable contract lives in `aaa.specification.en.md` and the package-owned `../template/pallets/aaa/EMBEDDING.md`; the current DEOS binding lives in `aaa.architecture.en.md`.

### 4.4 Amount Resolution

Execution-plan steps specify amounts via `AmountResolution`, enabling both static and dynamic resolution:

| Variant | Description | Use case |
| --- | --- | --- |
| `Fixed(Balance)` | Absolute amount | Known transfers |
| `PercentageOfCurrent` | % of current spendable balance | Gradual unwind |
| `PercentageOfTrigger` | % of trigger-time balance | Event-proportional |
| `PercentageOfLastFunding` | % of last funding snapshot | DCA repeats |
| `AllBalance` | Spendable balance | Burn/final transfer |

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

### 5.2 Direct-Route Deviation Validation and Pre-Execution Snapshot

On direct routes the router first validates the selected quote against the previously stored EMA. After that validation succeeds, it snapshots the current pool reserves into the EMA before executing the swap. The current trade therefore cannot update the EMA before its own deviation check. This is a bounded manipulation mitigation on those paths, not a complete flash-loan or MEV defense (multi-hop relies on slippage; no ordering/commit protection on this launch line).

```rust
pub fn swap(from: AssetKind, to: AssetKind, ...) -> DispatchResult {
    let route = Self::prepare_optimal_route(...)?; // Select route; validate slippage and direct-route deviation.
    Self::update_oracle_from_reserves(from, to)?; // Snapshot current pre-execution reserves only after validation.
    Self::execute_prepared_route(route, ...)?;
    Ok(())
}
```

### 5.3 Declarative Execution-Plan Pattern

AAA-managed economic automation is expressed as production-admitted execution plans:

```rust
// Burn Actor execution plan: swap all foreign → native, then burn
vec![
    Step { conditions: [BalanceAbove(foreign, dust)],
           task: SwapExactIn { foreign → Native, AllBalance, 5% slippage },
           on_error: ContinueNextStep },
    Step { conditions: [BalanceAbove(Native, dust)],
           task: Burn { Native, AllBalance },
           on_error: AbortCycle },
]

// BLDR Splitter execution plan: TMC routes collateral directly; actor splits BLDR
vec![
    Step { conditions: [BalanceAbove(BLDR, dust)],
           task: SplitTransfer { BLDR, AllBalance, 50%→Liquidity + 50%→Treasury },
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
- `Holding Register`: Incoming assets are held temporarily before dispatch to the asset transactor. The reference runtime caps this register at one asset while `FixedWeightBounds` is active, so one generated saturated foreign-asset AAA deposit envelope safely prices every instruction; multi-asset holding requires an instruction-specific weigher before activation.
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

The interaction of actors creates a conditionally bounded economy:

- `Ceiling`: Enforced by TMC. Multiple curves such as Native and BLDR each define independent ceilings.
- `Floor`: Reported from TOL bucket reserves that qualify under the TMCTOL floor metric and bucket-accounting rules. L1 support flows through AAA #3..#6; L2 BLDR support flows through AAA #12.
- `Compression`: Burn Actor execution reduces live supply and liquidity-actor LP provisioning can strengthen counted reserves. Bidirectional compression holds only under the named preconditions: protected counted reserves, explicit sellable-pressure assumptions, live burn execution, and healthy liquidity accounting.

### 7.2 Deflationary Velocity

The `Axial Router` acts as a vacuum for circulating supply.

- `Mechanism`: High base fee (e.g., 0.5%) + Protocol Priority Routing.
- `Outcome`: System value capture is prioritized over LP revenue. The protocol captures the spread to burn its own supply.

### 7.3 Multi-Token Flywheel

The BLDR economy creates a self-reinforcing loop:

```
User buys BLDR (via Router TMC)
  → NTVE collateral → BLDR Liquidity Actor → LP → BLDR Bucket A (floor ↑)
  → BLDR liquidity share → 50% to Liquidity Actor (more LP), 50% to Treasury (ecosystem fund)
  → Optional Bucket B LP transfer → Treasury B LP removal → buyback BLDR on XYK → burn (downstream policy target)
```

BLDR floor support and ceiling pressure can compress over time when LP accumulation remains counted as support and buyback-burn execution remains live.

## 8. Runtime Topology

### 8.1 Pallet Inventory

| Pallet | Role | Hooks |
| --- | --- | --- |
| `pallet-aaa` | Actor platform: 15 System + Users | `on_initialize`, `on_idle` |
| `pallet-axial-router` | Routing, fees, oracle | Extrinsic-driven |
| `pallet-tmc` | Multi-curve native emission | Extrinsic-driven |
| `pallet-asset-registry` | Foreign asset registry | Extrinsic-driven |
| `pallet-asset-conversion` | Uniswap V2-like AMM pools | Extrinsic-driven |
| `pallet-assets` | Fungible asset ledger | — |
| `pallet-balances` | Native token ledger | — |

### 8.2 Former Pallets (Consolidated into AAA)

| Former Pallet | Replacement | LOC Saved |
| --- | --- | --- |
| `pallet-burning-manager` | System AAA #0 (Burn Actor plan) | ~1100 |
| `pallet-zap-manager` | System AAA #2 (Liquidity Actor plan) | ~2200 |
| `pallet-treasury-owned-liquidity` | System AAA #3..#6 (4 bucket actors) | ~3100 |

Additional cleanup: the former TMC-to-liquidity adapter traits were removed from TMC (~150 LOC), superseded by `MintOutputResolver` + AAA execution-plan-driven routing.

## 9. Conclusion

The DEOS architecture transforms the blockchain from a passive ledger into an `Active Economic Automaton`.

By separating dedicated economic mechanisms from production-admitted `pallet-aaa` automation, the system provides:

1. `Bounded trade safety`: Direct routes combine slippage with pre-swap EMA deviation guards; multi-hop and full MEV/sandwich resistance are out of scope for this launch line (see Axial Router architecture).
2. `Composable Automation`: bounded AAA tasks and runtime adapters compose reconfigurable flows only when the resulting plan fits production admission and preserves custody/atomicity. New actor graphs avoid runtime code changes when existing primitives satisfy that contract; new mechanics require an admitted adapter or core-task review.
3. `Multi-Token Economy`: L1 (Native) and L2 (BLDR) economies operate independently with shared infrastructure. Each has its own TMC curve, liquidity pools, liquidity actor, and TOL buckets.
4. `Bounded Background Execution`: AAA uses budget-capped `on_idle`, bounded `on_initialize` bookkeeping, and durable carry-over rather than claiming zero congestion under every workload.
5. `Conditional Automation`: Activated, funded actors can burn, provision liquidity, or route treasury value when adapters, pools, triggers, and safety conditions remain healthy; dormant or unadmitted policy targets do not produce effects.
6. `Focused Ownership`: Dedicated pallets own minting, routing, staking, assets, and AMM mechanics while AAA owns bounded orchestration, avoiding duplicated manager loops without turning the actor kernel into a universal VM.

---

- `Last Updated`: July 2026
