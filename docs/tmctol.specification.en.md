# `TMCTOL` Standard Specification

## Abstract

TMCTOL (Token Minting Curve + Treasury-Owned Liquidity) is a tokenomic standard for DEOS, establishing mathematically defined price boundaries through treasury-controlled liquidity accumulation. The system combines unidirectional token emission with automated XYK reserve allocation to create calculable downside protection ranging from 11% to 25% of equilibrium price, contingent on governance maintaining specified system parameters.

`Key Properties`:

- Linear price ceiling via minting curve: `P_ceiling = P₀ + m·S_curve/PRECISION`
- XYK price model with two references: spot pool price `P_xyk = R_foreign/R_native` and stress-floor envelope `P_stress(x) = k/(R_native + x)²`, where `k = R_native × R_foreign`
- Supply compression through fee burning under explicit liveness assumptions
- Multi-bucket TOL architecture with explicit floor-reporting accounting

`Normative Scope`:

- `MUST` and `SHOULD` statements define conformance for TMCTOL realizations
- `Theorem`, `Analysis`, `Proof Sketch`, and scenario tables define mathematical or economic claims only under their stated preconditions
- Public floor statements MUST cite the canonical reported floor metric in Section 3.2 rather than quoting scenario tables without context
- Implementation-specific storage, queueing, and runtime wiring belong in paired architecture documents, but they MUST preserve the normative contract defined here

`2×2 Positioning Matrix (abbreviations only)`:

| Curve ↓ / Liquidity → | `POL` | `TOL` |
| --- | --- | --- |
| `TBC` | `TBCPOL` | `TBCTOL` |
| `TMC` | `TMCPOL` | `TMCTOL` |

`Glossary of Base Elements`:

- `TBC`: Bidirectional mint/redeem curve model, typically symmetric in formula-space (reserve-extraction path exists)
- `TMC`: Unidirectional mint-only curve model, inherently asymmetric in market structure (reserve-extraction path does not exist)
- `POL`: Protocol-Owned Liquidity — ownership class where LP inventory is held by protocol accounts; permanence depends on policy (hard-locked or governance-withdrawable)
- `TOL`: Treasury-Owned Liquidity — treasury policy framework over protocol-owned liquidity with explicit bucket roles; in TMCTOL, Bucket_A is the anchor permanence layer while B/C/D are policy-flex buckets

`Combination Profiles`:

- `TBC + POL`: Possible only with an explicit accumulation source such as fees, spread, surplus, or seed capital. Otherwise the redeem flow tends to consume reserves and POL cannot grow sustainably
- `TBC + TOL`: Adds treasury discipline and segmentation, but still requires dedicated LP inflow into treasury logic. The redeem path continues to create extraction pressure
- `TMC + POL`: Naturally supports reserve accumulation when mint-side allocations route into protocol LP. Floor quality then depends on treasury policy strictness
- `TMC + TOL`: Combines unidirectional minting with explicit treasury floor policy across Anchor, Building, Capital, and Dormant buckets. This maximizes floor hardness and governance clarity

`Relationship note`: TOL is a structured policy layer over POL. All TOL LP is POL by ownership, but not every POL design is TOL.

`Interpretation`: TMCTOL intentionally occupies the `TMC + TOL` quadrant to combine mint-side irreversibility with policy-explicit liquidity stewardship.

---

## 1. Standard Foundation

### 1.1 Core Innovation

TMCTOL addresses the fundamental problem of unlimited downside risk in token economics by establishing `mathematically bounded risk` through the interaction of three mechanisms:

1. `Unidirectional Minting Curve (TMC)`: Creates deterministic price ceiling without redemption option
2. `Treasury-Owned Liquidity (TOL)`: Automatically allocates 66.6% of mints to protocol-controlled XYK reserves
3. `Fee Burning Router`: Directs 0.5% of trading fees to supply compression

The framework transforms unbounded downside into a calculable range where floor protection depends explicitly on governance maintaining system conditions.

### 1.2 Governance Dependencies

All price protection properties are `conditional guarantees` requiring:

`Conditional Requirements`:

- TOL liquidity counted in the reported floor remains deployed in qualifying XYK reserves
- Distribution ratios are maintained per protocol specification: 66.6% TOL and 33.3% user allocation
- Fee burning remains live within the liveness contract in Section 6.1
- Router fee policy remains inside the bounded mutation range declared by the realization
- Bucket_A remains the protected anchor bucket under the invariant in Section 6.2
- Any governance action that removes a precondition MUST downgrade the reported floor state before or with the action

`Governance Risks`:

- Treasury withdrawal of TOL reduces effective floor proportionally
- Allocation ratio changes alter boundary calculations
- Emergency mechanism activation may bypass constraints
- Strategic bucket deployment affects realized floor level (11-25% range)

`Critical Distinction`: The framework provides "mathematically defined risk boundaries" rather than "absolute safety." Floor preservation represents a governance-dependent system property requiring continuous parameter enforcement. Traditional tokens exhibit unbounded downside regardless of governance quality; TMCTOL bounds this risk through transparent mathematical relationships contingent on maintaining specified conditions.

---

## 2. Technical Architecture

### 2.1 Core Type System

`Dimensional Types with PRECISION Scaling (10¹²)`:

- `Balance`: Native token quantities
- `Price [Foreign/Native]`: Exchange rates scaled by PRECISION
- `Slope [scaled Foreign/Native²]`: Linear emission rate parameter stored in `PRECISION`-scaled units
- `Perbill`: Parts-per-billion ratio (10⁹) for dimensionless quantities

`Purpose`: Type system encodes physical units preventing categorical errors. Price operations preserve dimensional correctness; ratio operations maintain scale separation.

### 2.2 Minting Curve Mathematics

`Linear Emission Model`:

```
spot_price(S_curve) = P₀ + m·S_curve/PRECISION

where:
  P₀ = initial_price, scaled as Foreign/Native
  m = stored slope parameter, scaled as Foreign/Native²
  S_curve = curve-accounted minted supply after initial issuance
```

`Quadratic Integration for Minting`:

```
Payment calculation integrates price curve:
F_required = P₀·ΔS + m·(S₀·ΔS + ΔS²/2)/PRECISION

where:
  F_required = foreign payment needed
  S₀ = supply before mint
  ΔS = tokens to mint
```

`Property`: Deterministic pricing provides a single quote for a given supply state and reduces pricing ambiguity.

### 2.3 Token Distribution

`Two-Way Split`:

- User allocation: 33.3% (immediate liquidity)
- TOL allocation: 66.6% (protocol reserves)

`Mint-Time Distribution Invariant`:

```
user_amount + tol_amount ≡ total_minted_in_transaction
```

Distribution occurs atomically within minting transaction; no newly minted tokens exist outside these two allocations at mint time. Later burns, transfers, LP operations, and treasury actions are accounted by separate ledger invariants rather than by this transaction-local equality.

### 2.4 Multi-Bucket TOL Architecture

`Bucket Structure (66.6% total allocation)`:

- `Bucket_A (33.3% total supply)`: Anchor liquidity — primary floor protection mechanism. Bucket_A is protected floor capital while it remains in an anchor-qualified state under Section 6.2. If Bucket_A exceeds its required anchor target due to supply compression or strategic accumulation, governance may migrate only the excess under the migration rules below
- `Bucket_B (11.1% total supply)`: Building budget — ecosystem construction spending through governed LP unwind, buyback, treasury, or deployment actions
- `Bucket_C (11.1% total supply)`: Capital bucket — operational liquidity reserve for controlled redeployment between LP positions and treasury balances
- `Bucket_D (11.1% total supply)`: Dormant LP reserve — governance-controlled LP held in a passive or delayed-use state until a later policy activates it

`Allocation vs. Circulating Share`: Initial bucket allocations represent fixed percentages of total supply at minting. However, circulating shares (percentage of current supply held by each bucket) evolve dynamically through token burning and strategic reallocations. This distinction enables the system to maintain floor protection guarantees while allowing treasury expansion.

`Capital Efficiency`: Four independent bucket positions target near-full reserve deployment rather than idle treasury custody. This does not imply one deep pool or identical user slippage across buckets. Any realization that advertises aggregate utilization MUST also report whether floor depth is concentrated, fragmented, migrated, or externally deployed.

`Floor Protection Range`: Effective floor varies based on bucket state:

- `Maximum model case (25%)`: All TOL buckets are in floor-supporting reserve state under the same stress metric
- `Minimum model case (11%)`: Only Bucket_A is in floor-supporting reserve state and other buckets are treated as sellable pressure or non-supporting deployment
- `Typical live state`: Governance balances protection, development, capital use, and external deployment; clients MUST report the current bucket-state classification rather than assuming either endpoint
- `Excess Liquidity Migration`: Token burning increases Bucket_A's relative share of circulating supply. Governance MAY migrate only Bucket_A liquidity above the required anchor target, and the migrated portion MUST stop counting toward in-domain floor support unless it satisfies the external LP reporting rules in Section 6.2

### 2.5 Axial Router Mechanism

`Price Discovery Gateway`:

- Compares TMC spot price against XYK pool pricing
- Routes trades to mechanism offering better execution
- Collects 0.5% fee (default) directed to burning
- Emits route type (TMC/XYK) for transparency

`Fee Structure`:

- Router fee: 0.5% default → 100% burned (supply compression)
- Router fee mutation MUST be bounded by an explicit maximum; the reference line caps governance updates at 1%
- XYK fee: 0.0% (default) → maximizes spread tightness
- Governance may activate XYK fees for additional deflation only under a declared bounded policy

`Critical Function`: Router ensures consistent price discovery while creating deflationary pressure through mandatory fee burning. Without router, arbitrage opportunities would exist between TMC and XYK pricing.

### 2.6 Zap Liquidity Mechanism

`Purpose`: Intelligent liquidity addition handling price imbalances between native/foreign reserves.

`Strategy`: When pool price diverges from fair value, Zap mechanism:

1. Calculates optimal split between native/foreign contributions
2. Swaps excess portion of imbalanced asset
3. Adds liquidity with balanced ratios
4. Maximizes LP tokens received per contribution

`Normative Output Contract`:

- Zap execution MUST define a maximum slippage bound and a maximum tolerated post-zap reserve-ratio error
- Zap execution MUST fail, defer, or enter a flagged degraded mode when those bounds cannot be met
- Initial pool seeding MUST have explicit rules for empty or one-sided pools
- A conforming realization SHOULD derive swap tolerance from native reserve depth, clamp it within explicit lower/upper bounds, and avoid a flat one-size-fits-all ceiling once deeper pools make tighter bounds materially safer
- The exact route-search and optimization strategy belongs in paired architecture/runtime docs, but the postconditions above are part of the TMCTOL contract

### 2.7 Fee Burning System

`Accumulation Phase`:

- Router collects fees in the configured fee asset or assets
- Fees accumulate in bounded buffers until an execution threshold is met
- Thresholds prevent dust burns wasting weight or fees

`Execution Phase`:

- Burn execution swaps accumulated fees for native tokens through an allowed route set
- Burn execution removes acquired native tokens from total issuance
- Burn execution updates transparent burned-supply accounting

`Liveness Contract`:

- A realization MUST define a maximum fee-buffer threshold or equivalent bounded trigger condition
- A realization MUST define retry, cooldown, and failure-reporting behavior for failed swaps or burns
- A realization MUST expose a degraded state when burn execution is unable to progress while fee buffers keep accumulating
- Continuous ratchet equations in this document are analytical approximations; conformance MUST also cover discrete batch execution and bounded approximation error

`Supply Dynamics`: Burning creates bidirectional compression—ceiling decreases (fewer tokens at given slope), floor increases (fixed reserves divided by smaller supply). This accelerates convergence toward equilibrium.

---

## 3. Mathematical Foundations

### 3.1 XYK Constant Product Necessity

`Mathematical Basis`:

```
XYK Invariant: k = R_native × R_foreign (constant)

After selling ΔS native tokens:
R_native' = R_native + ΔS
R_foreign' = k / R_native'

Price = R_foreign' / R_native' > 0 for all finite ΔS
```

`Critical Property`: Foreign reserves approach zero asymptotically but never reach zero. This mathematical guarantee underlies floor protection—price can deteriorate indefinitely but never reaches zero value.

`Comparison with Concentrated Liquidity`:

Concentrated liquidity depletes at specific thresholds:

```
Depletion point: θ = 1/(1 + A^(1/3))

A = 10:  reserves depleted at 24% price drop
A = 50:  reserves depleted at 15.7% price drop
A = 100: reserves depleted at 9.1% price drop
```

`Analysis`: Constant product maintains non-zero reserves under all price deterioration scenarios. Concentrated liquidity exhibits discontinuous reserve behavior where floor protection completely fails beyond depletion threshold. XYK's "inefficiency" (wider spreads) is precisely its strength for floor protection mechanisms.

### 3.2 Price Boundaries

`Ceiling Definition`:

```
P_ceiling(S) = P₀ + m·S/PRECISION

Properties:
- Monotonically increasing with supply
- Deterministic (no market dependency)
- Governance-invariant (only changes via parameter modification)
```

`Floor Model (two distinct references)`:

```
k = R_native × R_foreign (constant product invariant)

Spot pool price:
P_xyk = R_foreign / R_native

Stress-floor envelope for selloff size x:
P_stress(x) = k / (R_native + x)²
```

`Canonical Reported Floor Metric`:

The public TMCTOL floor metric is the stress-floor ratio against the parity reference ceiling for a named bucket state:

```
reported_floor_ratio = P_stress(x_reported) / P_ceiling_ref
P_ceiling_ref = P_xyk_ref = R_foreign / R_native
x_reported = λ_reported · S_circ_support_scope
```

A conforming report MUST state:

- `Reserve scope`: which in-domain and external reserves are counted
- `Bucket state`: which bucket balances are anchor, active support, dormant, migrated, deployed, or spent
- `Supply basis`: whether `S_total`, `S_curve`, or `S_circ` is used
- `Sellable pressure`: the chosen `λ_reported` assumption and rationale
- `Governance state`: intact, degraded, emergency, or forked guarantee surface

`Reference-state assumption for ratio estimates`:

To express scenario estimates using only `s` and `a`, we normalize at a parity reference state:

```
P_ceiling_ref = P_xyk_ref = R_foreign / R_native
a = R_native / S_total
s = S_sold / S_total

Then:
P_stress / P_ceiling_ref = 1 / (1 + s/a)²
```

These scenario estimates are not standalone public guarantees unless the report also supplies the canonical metric fields above.

`Scenario Analysis` (assuming `a = 33.3%` Base Support):

- `User Exit`: public allocation sold, `s = 0.333`, stress/ceiling ratio `25%`, volatility `4×`
- `System Exit`: public plus Treasury buckets B/C/D sold, `s = 0.667`, stress/ceiling ratio `11%`, volatility `9×`

`Key Dependency`:

- `User Exit`: Represents total selling of initial public supply. With Bucket_A support (`a=33%`), stress floor remains at 25% of reference ceiling.
- `System Exit`: Represents a catastrophic scenario where Treasury buckets (B, C, D) enter circulation and are also sold. Under maintained protocol assumptions, Bucket_A implies an approximately 11% modeled stress-floor ratio in this case.

### 3.3 Ratchet Effect Analysis

`Mechanism`: Floor elevation occurs through asymmetric component interaction:

`Component Dependencies`:

1. `TMC Pricing`: Linear supply curve `P(S) = P₀ + m·S/PRECISION`
2. `TOL Allocation`: Fixed reserve ratio maintained by governance
3. `Supply Burning`: Fee-driven compression `dS/dt = -f·V_trade`

`Supply Symbols`:

- `S_curve`: curve-accounted minted supply after initial issuance, used by TMC pricing
- `S_total`: total issued native supply before applying sellable-pressure assumptions
- `S_circ`: circulating or economically sellable supply after excluding explicitly locked or non-sellable balances under the reporting policy
- `x_max`: modeled sellable pressure
- `λ`: sellable-pressure fraction, selected by scenario or reporting policy

`System Interaction`:

When burning reduces `S_circ` by `ΔS`, the stress-envelope improves because maximum sellable inventory contracts.

Define sellable pressure as `x_max = λ · S_circ`, where `λ ∈ (0, 1]` is governance/market dependent and MUST be named in any public floor report.

1.  `Supply Contraction`: `S'_circ = S_circ - ΔS`
2.  `Stress-Floor Elevation`: reduced `x_max` raises the stress envelope.
    ```
    P'_stress,max = k / (R_native + λ·S'_circ)² > k / (R_native + λ·S_circ)²
    ```
3.  `Ceiling Depression`: minting price lowers as supply retracts.
    ```
    P'_ceiling = P(S'_circ) < P(S_circ)
    ```
4.  `Spot Price Note`: if reserves are unchanged, `P_xyk = R_foreign/R_native` is unchanged
5.  `Result`: the stress corridor compresses from both sides (Bidirectional Compression).

`Stress-Floor Elevation Velocity (holding k, R_native, λ fixed over dt)`:

```
dP_stress,max/dt ∝ (burn_rate × λ × k) / (R_native + λ·S_circ)³

Properties:
- Velocity is proportional to burn rate (governance controls via fees)
- Velocity increases as sellable inventory shrinks
- Relationship is nonlinear due to cubic denominator
```

`Phase Evolution`:

`Phase 1 — Early Accumulation` (low TOL/supply ratio):

- TMC pricing dominates, XYK spreads wide
- Floor elevation rate: low
- System fragile, high volatility

`Phase 2 — Transition` (moderate TOL/supply ratio):

- Both mechanisms contribute comparably
- Floor elevation accelerates
- Bootstrap gravity well forms (~15% TOL/market-cap)

`Phase 3 — Maturation` (high TOL/supply ratio):

- XYK mechanism dominates
- Convergence to equilibrium
- System exhibits stability

`Governance Contingency`: Ratchet operates only when governance maintains:

1. TOL reserve allocations (prevents drainage)
2. Fee burning mechanism (enables supply compression)
3. Distribution ratios (ensures TOL accumulation)

Reversal requires governance decisions: reserve withdrawal, fee deactivation, or allocation changes. Floor elevation represents state-dependent dynamics, not irreversible progression.

### 3.4 Bidirectional Compression

`Supply Burning Effects`:

```
TMC pricing exhibits explicit supply dependence:
P_TMC(S) = P₀ + m·S/PRECISION

When supply decreases by ΔS:
ΔP_ceiling = -m·ΔS/PRECISION (ceiling compression)
P_xyk = const(R_native, R_foreign)
        (spot pool price unchanged if reserves stay constant)
P_stress,max = k / (R_native + λ·S_circ)²
              (stress floor rises as sellable inventory contracts)

Net effect: stress corridor compression with floor-ceiling convergence
```

`Progression Example` (R_foreign = 666,667 Foreign, m = 1,500,000):

| Supply | P_ceiling | P_stress (min) | Spread |
| --- | --- | --- | --- |
| 1M | 1.501 | 0.11 | 13.6× |
| 500k | 0.751 | 0.22 | 3.4× |
| 200k | 0.301 | 0.56 | 0.54× |

`Critical Point`: When `P_stress > P_ceiling`, arbitrage incentives reverse. Minting becomes more attractive than market selling, creating natural equilibrium.

`Threshold Taxonomy`: Four distinct boundaries and two compression metrics must not be conflated.

1. `Elasticity Inversion Threshold`

```
dF/dS = 0
```

In the simulator's expanding-supply framing, the effective stress floor can be written as:

```
F(S) = k(S) / S²
k(S) = R_native(S) × R_foreign(S)
```

Then:

```
dF/dS = (S × k'(S) - 2k(S)) / S³
```

So inversion occurs when:

```
S × k'(S) = 2k(S)
d ln k / d ln S = 2
```

`Interpretation`: This is only the boundary where the floor stops deteriorating under supply expansion. At the threshold itself the floor is flat while the ceiling still rises linearly, so inversion does not mean the floor is already compressing the corridor.

2. `Relative Compression Parity Threshold`

This threshold asks whether the ratio-based corridor is compressing under supply expansion.

```
G_mult(S) = C(S) / F(S)
d ln F / d ln S = d ln C / d ln S
C(S) = P₀ + m·S/PRECISION
ε_C = d ln C / d ln S ∈ (0,1]
```

For `F(S) = k(S) / S²`:

```
d ln F / d ln S = d ln k / d ln S - 2
```

So relative-parity requires:

```
d ln k / d ln S = 2 + ε_C
```

At large `S`, where `P₀` contributes little, `ε_C ≈ 1`, so relative parity is approximately:

```
d ln k / d ln S ≈ 3
```

`Interpretation`: Inversion corresponds to `k` growing like `S²`; relative compression parity requires `k` growing closer to `S³`.

3. `Absolute-Gap Compression Threshold`

If compression is measured as the arithmetic spread:

```
G_abs(S) = C(S) - F(S)
```

then parity and compression require:

```
dG_abs/dS = dC/dS - dF/dS
absolute-gap parity: dF/dS = dC/dS
absolute-gap compression: dF/dS > dC/dS
```

`Interpretation`: A floor can rise while still growing more slowly than the ceiling in absolute price units. In that regime the floor is recovering, but the absolute gap is still widening.

4. `Arbitrage Reversal / Overtake Threshold`

```
F(S) ≥ C(S)
P_stress ≥ P_ceiling
```

`Interpretation`: This is stronger than inversion and stronger than either compression metric. Here the floor has caught or exceeded the ceiling enough to reverse the dominant incentive structure.

`Metric and Axis Clarification`:

- Section `3.4 Bidirectional Compression` is a burn-time claim: along sustained burning, `dC/dt < 0` while `dF/dt > 0`, so the corridor compresses directly
- This threshold taxonomy instead uses an expanding-supply framing, asking what happens as `S` increases
- The progression table above expresses `Spread` as a multiplier (`13.6×`, `3.4×`, `0.54×`), so relative compression is the native reading of that table unless absolute gap is named explicitly
- Therefore post-inversion floor growth does not by itself imply corridor compression; compression depends on whether the chosen metric is multiplicative (`C/F`) or absolute (`C-F`)

`Operational Regime Map`:

- `d ln k / d ln S < 2`: floor falls
- `d ln k / d ln S = 2`: inversion threshold; floor is flat
- `2 < d ln k / d ln S < 2 + ε_C`: floor rises again, but relative compression has not started
- `d ln k / d ln S = 2 + ε_C`: relative compression parity; `C/F` is flat
- `d ln k / d ln S > 2 + ε_C`: relative compression; `C/F` narrows
- `dF/dS = dC/dS`: absolute-gap parity; `C - F` is flat
- `dF/dS > dC/dS`: absolute-gap compression; `C - F` narrows
- `F(S) ≥ C(S)`: arbitrage reversal / overtake

`Test Scope Clarification`: The simulator's `Supply Elasticity Inversion Point` test targets the first threshold only. It validates that floor deterioration can stop and reverse after a critical boundary; it does not by itself prove relative compression, absolute-gap compression, or immediate floor/ceiling crossover.

### 3.5 Equilibrium Analysis

`Backing Equilibrium`:

The price point where the Market Cap implied by the Curve (`P·S`) is fully backed by the Foreign Reserve (`R_foreign`).

```
P_backing ≈ √(R_foreign × m / PRECISION)
```

`Parity Equilibrium`:

The instantaneous price parity point where the curve and XYK quote the same marginal price.

```
P_parity = P_curve(S) = P_xyk = R_foreign / R_native
```

`Equilibrium Relationship`:

- `P_backing` is the canonical backing reference in this document (`P · S ≈ R_foreign`)
- `P_parity` is the mechanism parity reference for routing and execution paths
- The two references can diverge at a given state and converge through market activity plus burning

`Dimensional Validation`:

```
√([Foreign] × [Foreign/Native²]) = √([Foreign²/Native²]) = [Foreign/Native] = [Price]
```

`Significance`:

- `Gravity Well`: Price oscillates around this value as volatility stabilizes.
- `Router Behavior`: Below `P_backing`, supply is "oversold" (heavy floor support). Above `P_backing`, supply is "premium" (utility driven).

`Numerical Example`:

```
R_foreign = 1,000,000 foreign tokens
m = 1,000,000,000 (slope in PRECISION units)
PRECISION = 10¹²

P_backing ≈ √(1,000,000 × 1,000,000,000 / 10¹²)
    = √1,000
    ≈ 31.62 Foreign per Native
```

`Interpretation`: `P_backing` is the backing-reference level where curve-implied market cap equals foreign reserves. It is distinct from instantaneous mechanism parity (`P_parity`), while convergence dynamics remain burn-rate dependent.

`Governance Dependency`: `P_backing` explicitly depends on `R_foreign` and `m` parameters. Changes to TOL allocation alter this backing target. In the default TMCTOL launch contract, curve launch parameters such as `m` are immutable after curve creation; changing them is a fork or runtime-upgrade extension that owns a different guarantee surface.

---

## 4. Economic Model

### 4.1 Supply Dynamics

`Emission`: Unidirectional minting creates monotonically increasing supply ceiling. No redemption mechanism prevents reserve drainage.

`Compression`: Fee burning creates deflationary pressure. Net supply trajectory depends on mint rate versus burn rate:

```
dS/dt = mint_rate - burn_rate
where burn_rate = f_router × V_trade
```

`Capital Efficiency`: Multi-bucket TOL achieves ~100% capital utilization through deployment cycles:

- `Traditional treasury`: 0% (holds unbacked tokens in vaults)
- `Single pool TOL`: ~50% (capital locked in single pool)
- `Four-bucket TOL`: ~100% (continuous XYK liquidity deployment with temporary buffer recycling, varied governance thresholds)

`Flexibility`: Bucket independence enables:

- `Bucket_A`: Dedicated baseline floor support target; governance policy should treat it as protected capital and minimize withdrawal paths
- `Buckets 2-4`: Strategic deployment per governance decisions
- Effective floor ranges 11% minimum (only Bucket_A) to 25% maximum (all buckets) based on deployment choices

### 4.2 Infrastructure Premium

`Theorem`: For equal liquidity depth, protocol-owned liquidity provides better execution than mercenary LP capital.

`Proof Sketch`:

1. Mercenary LPs extract fees (dilute reserves over time)
2. Protocol TOL grows from mints (accumulates over time)
3. For equal starting liquidity, TOL provides tighter spreads long-term

`Implication`: Zero XYK fees (default) optimal when TOL provides all liquidity. No need to compensate external LPs; protocol benefits from tight spreads and user convenience.

### 4.3 Value Flows

`Minting Flow`:

```
Foreign payment → TMC calculation → Token emission
                                       ↓
                          User (33.3%) + TOL (66.6%)
                                            ↓
                          TOL → Multi-bucket distribution → XYK pools
```

`Trading Flow`:

```
Trade request → Router price comparison → Route selection
                                            ↓
                            TMC (if better) or XYK (if better)
                                            ↓
                            Fee collection (0.5%) → Foreign buffer → Burn execution
```

`Burning Flow`:

```
Foreign fees accumulate → Threshold reached → Swap to native → Burn
                                            ↓
                        Supply decreases → Ceiling compresses + Floor elevates
```

---

## 5. System Dynamics

### 5.1 Virtuous Cycle

```
Adoption → Mints → Higher ceiling + More TOL
    ↓                             ↓
Activity ← Trading ← Higher floor (stronger support)
    ↓
Burning ← Fees ← Volume
    ↓
Narrower range → Reduced volatility → Increased confidence → Adoption
```

`Feedback Mechanisms`:

- Positive: Adoption drives minting → TOL accumulation → stronger floor → confidence → adoption
- Negative: High volatility → reduced confidence → lower adoption → slower TOL growth
- Stabilizing: Floor elevation → reduced downside → risk-adjusted returns improve

### 5.2 Systemic Behavior

`Equilibrium Regions`:

- `Floor proximity`: Reduced arbitrage opportunity; awaiting catalyst
- `Equilibrium band`: Balanced forces; stable trading reflects fair value
- `Ceiling proximity`: Minting incentivized; emission accelerates

`Volatility Dynamics`: Stress spread compression decreases monotonically over burn time only under the named metric and preconditions: sustained burning, fixed counted reserves, stable `λ`, and no governance action that removes floor-supporting liquidity. If the metric is absolute spread (`P_ceiling - P_stress,max`) rather than relative spread (`P_ceiling / P_stress,max`), the report MUST name that choice explicitly.

### 5.3 Evolution Path

`High-Volatility Development Phase`:

- Wide price range characteristic of early systems
- Floor building via TOL accumulation
- Price discovery through market mechanisms

`Maturing Growth Phase`:

- Governance deploys TOL strategically (parachain expansion, development funding)
- Framework flexibility enables growth without sacrificing floor protection
- Burn effects create deflationary pressure

`Matured Ecosystem Phase`:

- Stability emerges from accumulated TOL depth
- Range compression approaches equilibrium
- Governance maintains deployment flexibility

`Advanced Stability Phase`:

- Narrow range achieved through bidirectional compression
- Price converges to √(R_foreign × m / PRECISION)
- System exhibits "rising stability asset" properties

---

## 6. Implementation Requirements

### 6.1 Technical Implementation

`TOL Reserve Management`:

- Treasury controls allocation via governance
- Withdrawal requires consensus and MUST NOT be possible through unilateral admin keys in the default guarantee surface
- Multi-bucket structure uses independent LP positions or explicitly mapped accounting positions
- Share-based accounting MUST prevent pool-state changes from corrupting bucket ownership
- Any reserve movement that reduces counted floor support MUST update the floor-reporting state in the same governance/runtime release path

`XYK Mechanism`:

- Constant product formula necessary for floor properties
- Pool initialization via Zap mechanism handling imbalances
- Slippage protection prevents value extraction
- Reserves transparent and verifiable on-chain

`Fee Routing`:

- Router collects the configured default fee, currently 0.5% in the reference standard
- Fee mutability MUST follow the parameter table below
- Fee buffers accumulate only under bounded liveness rules
- Burn execution swaps to native and removes supply
- Total burned MUST be tracked or derivable for transparency

`TMC Launch Physics`:

- Curve launch parameters are configured at `create_curve`
- The default TMCTOL launch contract treats those launch parameters as immutable after launch
- Forks MAY widen that surface deliberately, but then own the changed economic risk rather than inheriting the default TMCTOL guarantee surface

`Precision Requirements`:

- `PRECISION = 10¹²` for price and stored slope values
- Stored slope is already scaled; formulas MUST NOT apply an extra hidden precision division beyond the stated `m·S/PRECISION` form
- `PPB = 10⁹` for dimensionless ratios
- Multiplication before division MUST use a width that cannot overflow for the configured asset bounds
- Rounding direction MUST be specified for mint quotes, mint execution, bucket splits, fee collection, and burn execution
- Dimensional correctness SHOULD be enforced by types or conformance tests

`Parameter Mutability`:

- `Curve launch parameters`: immutable after `create_curve` in the default TMCTOL launch contract
- `TOL user/bucket split ratios`: immutable after launch unless a fork or explicit runtime-upgrade standard revision changes the guarantee surface
- `Router fee`: bounded-mutable by governance only when the bounds, authority, and effective delay are specified
- `XYK fee`: bounded-mutable by governance only when the floor and best-execution impact is disclosed
- `Burn threshold and cooldown`: bounded-mutable operational parameters; changes MUST preserve the burn liveness contract
- `Zap slippage bounds`: bounded-mutable operational parameters; changes MUST preserve the Zap postconditions
- `Bucket migration policy`: governance-mutable only for non-anchor or anchor-excess liquidity; protected anchor liquidity follows the TOL Anchor Invariant
- `Emergency controls`: unavailable by default unless a realization defines scope, duration, authority, veto/timelock, and reporting semantics

### 6.2 Critical Invariants

`Mint-Time Conservation`:

```
user_amount + tol_amount ≡ total_minted_in_transaction
```

Violation indicates distribution calculation error.

`Ledger Conservation with Burns`:

```
initial_issuance + cumulative_minted - cumulative_burned = live_issuance
```

Realizations MAY express this as a derived invariant when the runtime can derive burned supply from issuance history.

`TOL Anchor Invariant`:

Bucket_A counted as protected anchor support MUST satisfy all conditions below:

- It is held in protocol-owned or treasury-owned accounts governed by the default protected governance surface
- It is deployed in an in-domain XYK pool or another explicitly qualifying floor-supporting reserve position
- It is held by a System Immutable AAA or equivalent hard protocol anchor that runtime extrinsics, including governance/root, cannot mutate, pause, close, or reopen
- It cannot be withdrawn, migrated, spent, or reclassified by unilateral admin authority
- Any migration is limited to anchor-excess liquidity above the required anchor target unless a runtime upgrade, fork, or explicit standard revision degrades the guarantee surface
- Any emergency path that can bypass this invariant MUST classify the floor state as emergency or degraded before users can rely on the former reported floor

`Bucket and LP Accounting`:

Each bucket balance MUST be classifiable into exactly one live state for floor reporting:

- `Anchor support`: protected Bucket_A liquidity that satisfies the TOL Anchor Invariant
- `Active support`: non-anchor bucket liquidity currently counted in the reported floor metric
- `Dormant LP`: LP held without active spending policy but not necessarily counted as anchor support
- `Migrated LP`: liquidity deployed outside the primary in-domain pool set
- `Treasury liquid`: assets withdrawn from LP and held for spending or redeployment
- `Spent`: assets no longer controlled by the TOL bucket

Only `Anchor support` and explicitly reported `Active support` count toward the canonical reported floor. `Migrated LP` counts only when the report states the external venue, liquidity ownership, withdrawal rules, and stress model.

`Constant Product`:

```
k = R_native × R_foreign (before) ≈ k' (after fees)
```

XYK trades preserve k within fee tolerance.

`Non-Negative Reserves`:

```
R_native > 0 and R_foreign > 0 always
```

Reserve depletion would break floor guarantee.

`Monotonic Ceiling`:

```
P_ceiling(S_curve₂) ≥ P_ceiling(S_curve₁) for S_curve₂ ≥ S_curve₁
```

For a fixed curve, the ceiling is monotonic in curve-accounted minted supply. Burn-time ceiling compression is a separate statement about lower live supply or reporting basis, not about mutating the curve function.

### 6.3 Conformance Matrix

A TMCTOL realization MUST map every public claim to one of these states:

- `Normative and tested`: required behavior with conformance tests
- `Normative and inspected`: required behavior covered by runtime inspection or static validation
- `Analytical only`: theorem or model with named preconditions, not an implementation requirement
- `Commentary`: explanatory prose with no conformance claim

Current required coverage:

- `Mint-time distribution`: normative and tested
- `Ledger conservation with burns`: normative and tested or inspected
- `TOL Anchor Invariant`: normative and inspected through governance/storage/accounting surfaces
- `Canonical reported floor metric`: normative and tested for representative bucket states
- `Burn liveness`: normative and tested for threshold, retry, and degraded reporting paths
- `Zap postconditions`: normative and tested with initialization and imbalanced-pool vectors
- `Elasticity inversion`: analytical unless the simulator/runtime test suite explicitly covers the selected formula and parameters
- `Relative compression, absolute-gap compression, and overtake`: analytical until each regime has dedicated conformance vectors

A conforming reference runtime SHOULD expose a bounded live guarantee-state projection for inspection. This projection MUST NOT add dashboard/history storage; it MAY report uninitialized or explicitly non-guaranteed classes separately from violations. Native burn liveness and BLDR buyback/burn liveness MUST be reported as separate domains when both exist. Zap postcondition inspection MUST verify the configured liquidity-add, residual swap, and LP bucket split as one coherent plan before reporting the Zap domain as satisfied.

### 6.4 Economic Conditions

`Utility Requirement`: Token demand must derive from genuine use cases. Without utility, downside protection mechanisms operate in isolation without recovery catalyst.

`Continuous Development`: Protocol improvements maintain competitive positioning. Floor provides time for development; recovery requires utility delivery.

`Market Dynamics`: Long-term holders implicitly accept volatility during maturation. Floor protection bounds downside; upside depends on utility and adoption.

`Transparent Communication`: Fee structures and allocation formulas disclosed. Governance decisions visible to participants. Floor protection explicitly marked as governance-dependent.

---

## 7. Advantages & Trade-offs

### 7.1 Framework Strengths

`Mathematically Defined Boundaries`:

- Floor: 11-25% scenario range under the canonical reported floor metric and intact preconditions
- Ceiling: Deterministic via immutable launch curve in the default TMCTOL contract
- Both verifiable through transparent formulas plus bucket-state and reserve accounting

`Governance Flexibility`:

- Multi-bucket enables balancing protection vs. development
- Operational parameters are adjustable only within their declared mutability class
- Deployment strategies are adaptable when reports stop counting moved liquidity as protected floor support unless it still qualifies

`Capital Efficiency`:

- Near-full reserve deployment is possible through independent bucket structure with buffer recycling
- No mercenary capital requiring yield incentives is required for protocol-owned liquidity
- Protocol owns liquidity, governance controls strategy, and clients must distinguish aggregate utilization from concentrated executable depth

### 7.2 Framework Limitations

`Governance Dependencies`:

- Floor protection requires continuous parameter enforcement
- Reserve withdrawal reduces effective floor proportionally and must downgrade reports
- Emergency mechanisms are out of the default guarantee surface unless fully specified by scope, authority, duration, veto/timelock, and recovery reporting

`Market Dependencies`:

- Recovery from floor requires market confidence in utility
- Floor provides opportunity but not guarantee of appreciation
- Arbitrage creates mechanism but not obligation for recovery

`Complexity Trade-offs`:

- Multi-bucket structure adds governance overhead
- Zap mechanism requires sophisticated liquidity management
- Router requires maintenance of price discovery infrastructure

---

## 8. Summary

TMCTOL establishes a framework with mathematically derived price relationships and governance-controlled parameters:

`Core Mechanisms`:

- Unidirectional minting via linear curve creates deterministic ceiling
- 66.6% TOL allocation to XYK reserves establishes conditional hyperbolic floor support
- Fee burning compresses supply when burn liveness holds
- Multi-bucket architecture enables governance flexibility between protection and development with explicit floor accounting

`Mathematical Framework`:

```
Ceiling:         P_ceiling = P₀ + m·S/PRECISION
Spot XYK:        P_xyk = R_foreign / R_native
Stress Floor:    P_stress(x) = k / (R_native + x)²
Backing Eq:      P_backing ≈ √(R_foreign × m / PRECISION)
Parity Eq:       P_parity = R_foreign / R_native
Stress Velocity: dP_stress,max/dt ∝ (burn_rate × λ × k)/(R_native + λ·S_circ)³
```

`Critical Dependencies`:

1. Governance maintains TOL allocation: 66.6% of mints
2. Bucket distribution is enforced and Bucket_A satisfies the TOL Anchor Invariant
3. Fee burning mechanism satisfies the liveness contract
4. Counted reserves are protected from withdrawal without protected-governance consensus
5. Public reports classify bucket state, supply basis, reserve scope, and sellable pressure

`Framework Boundaries`:

Floor protection operates within the modeled 11-25% scenario range only when the canonical reported floor metric fields are stated:

- Maximum model case (25%): all buckets counted as support under the same stress metric
- Minimum model case (11%): only Bucket_A counted as support while other buckets are non-supporting or sellable pressure
- Governance controls the trade-off between floor strength and ecosystem development, but moved or spent liquidity no longer counts unless it still satisfies reporting rules

`Realization Requirements`:

System exhibits predicted dynamics (floor elevation, range compression) only when governance sustains parameters. Downside risk remains bounded by mathematical relationships. Upside recovery depends on protocol delivering utility and maintaining market confidence. Framework provides quantifiable risk parameters rather than absolute guarantees.

---

- `Version`: 0.4.0
- `Date`: May 2026
- `Author`: LLB Lab
- `License`: MIT
