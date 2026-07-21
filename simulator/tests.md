# TMCTOL Tests Mirror

Comprehensive test suite detalization synchronized with `/simulator/tests.js`

---

## Test Structure Overview

- Total Tests: 66
- Purpose: Formal verification of TMCTOL mathematical guarantees, component behavior, and emergent properties
- Methodology: Mathematical Foundations → Core Components → Integration Flows → System Properties → Economic Security
- Architecture Symmetry: Tests mirror TMCTOL system architecture for progressive comprehension

---

## 1. Mathematical Foundations

Core mathematical formulas and computational correctness.

### 1.1 Absolute Slope Formula Verification

- Nature: Direct formula validation against specification
- Necessity: Confirms `price = initial_price + slope × supply / PRECISION` holds exactly
- Validates: Linear price progression, zero-supply initial price, correct dimensional scaling
- Failure Criteria: Price calculation deviates from formula by >1 wei

### 1.2 Quadratic Integration for Minting

- Nature: Calculus-based mint amount verification
- Necessity: Proves integral-based token calculation matches theoretical quadratic formula
- Validates: `mint = payment / avg_price` where `avg_price = (price_before + price_after) / 2`
- Failure Criteria: Mint amount error >0.01% from theoretical integral

### 1.3 Linear Price Doubling Property Verification

- Nature: Tests perfect linear scaling property for specific parameter configurations
- Necessity: Validates that when `price_initial = PRECISION/N` and `slope = PRECISION/N`, the system exhibits ideal linear behavior where doubling supply approximately doubles price
- Validates: Price ratio equals supply ratio within 1% tolerance for linear function `P(S) = (PRECISION + S) / N`; system maintains linear scaling across multiple supply points; linear approximation holds when `S >> PRECISION`
- Failure Criteria: Price doubling ratio deviates >1% from supply doubling ratio; non-linear behavior in linear parameter configuration
- Key Insight: Perfect linearity emerges when initial price and slope share identical scaling factors, enabling predictable price progression

### 1.4 Zero Slope (Constant Price)

- Nature: Degenerate case where `slope = 0`
- Necessity: Confirms system degrades gracefully to fixed-price model
- Validates: Price independence from supply when slope disabled

---

## 2. System Parameters & Scaling

Parameter boundaries, scaling rules, and precision validation.

### 2.1 Initial Price Boundary Testing

- Nature: Tests `price_initial` from 1 wei to millions
- Necessity: Ensures system stability across entire economic range
- Validates: Minimum (1 wei), fractional (0.000001), standard (1.0), extreme (1M+)

### 2.2 Slope Boundary Testing

- Nature: Tests `slope` from 0 to extreme values with PRECISION scaling
- Necessity: Confirms pricing model works for flat, gentle, and aggressive slopes
- Validates: Zero (constant), minimal (0.000001), standard (0.001), high (0.1), extreme (1.0+)

### 2.3 Supply Boundary Testing

- Nature: Tests behavior from zero to billions of tokens
- Necessity: Proves formula stability under hyperscale token economies
- Validates: Empty state, small (1K), medium (1M), large (1B+)

### 2.4 Large Number Stress Test

- Nature: Tests near uint256 maximum values
- Necessity: Prevents overflow vulnerabilities in production
- Validates: 128-bit operations, maximum safe minting amounts

### 2.5 Parameter Combination Testing

- Nature: Tests extreme parameter pairings
- Necessity: Reveals edge cases invisible in isolated testing
- Validates: Low price + high slope, high price + zero slope, etc.

### 2.6 Current Default Parameters Validation

- Nature: Smoke test for production configuration
- Necessity: Ensures DEFAULT_CONFIG represents sane, tested values
- Validates: All defaults produce mathematically valid system state

---

## 3. Scaling Rules & Precision

Precision model verification and scaling rule consistency.

### 3.1 Scaling Rules - Naming Convention

- Nature: Structural test for scaling convention adherence
- Necessity: Enforces self-documenting code pattern
- Validates: Fractional values (fees, shares) use `_ppb` suffix; slope uses PRECISION scaling without suffix

### 3.2 Scaling Rules - Input Pre-scaling

- Nature: Confirms inputs arrive scaled to correct units
- Necessity: Prevents double-scaling bugs
- Validates: Price and slope use PRECISION; amounts use PRECISION; percentages (fees, shares) use PPB

### 3.3 Scaling Rules - Price Scaling Consistency

- Nature: Validates price and slope dimensional consistency
- Necessity: Ensures all price-related calculations maintain [Foreign/Native] × PRECISION scaling
- Validates: Price formula P(s) = P₀ + slope·s/PRECISION produces consistent units

### 3.4 Scaling Rules - PPB Values Range

- Nature: Validates PPB values sum to 1,000,000,000 (100%)
- Necessity: Prevents distribution math errors
- Validates: Share allocation totals exactly 100%

### 3.5 Scaling Rules - Precision Through Calculations

- Nature: Tests precision loss through calculation chains
- Necessity: Quantifies rounding error accumulation
- Validates: Errors remain within acceptable tolerance (<0.01%)

### 3.6 Scaling Rules - Property Fuzz

- Nature: Property-based validation of price/slope scaling
- Necessity: Ensures exact adherence to dimensional scaling across randomized inputs
- Validates: `P(S) = P₀ + m·S/PRECISION` holds exactly; price remains non-negative; inputs conform to PRECISION/PPB domains
- Failure Criteria: Any deviation from the formula; negative price; unit-inconsistent results

---

## 4. Core Component Tests

Isolated validation of individual system components in architectural order.

### 4.1 System Initialization

- Nature: Tests system factory and component wiring
- Necessity: Confirms dependency injection creates valid state
- Validates: All components initialized, cross-references correct

### 4.2 TMC Minting and Distribution

- Nature: Tests token creation and share allocation
- Necessity: Proves distribution percentages match configuration
- Validates: User/TOL shares exact (33.3%/66.7%), supply increases correctly, TOL manages 4 internal buckets

### 4.3 TOL Adding Liquidity to XYK

- Nature: Tests automatic liquidity provision via Zap
- Necessity: Confirms TOL accumulation mechanism functions
- Validates: LP tokens minted, reserves increased, ratios preserved

### 4.4 XYK Pool Swaps

- Nature: Tests constant-product AMM formula
- Necessity: Validates `x × y = k` invariant preservation
- Validates: Swap calculations, fee collection, reserve updates

### 4.5 XYK Multi-Swap Invariant

- Nature: Validates constant-product `x × y = k` under multiple sequential swaps
- Necessity: Ensures invariant stability under repeated activity; guards against pathological drift
- Validates: `k` is non-decreasing within fee/rounding tolerance on each step; reserves remain strictly positive

### 4.6 XYK Fee Tracking - Native to Foreign

- Nature: Tests fee calculation and tracking for native-to-foreign swaps
- Necessity: Validates that foreign_xyk_fee is correctly computed and returned
- Validates: Fee field returned in swap result, fee is non-negative, fee equals zero when fee_ppb is 0
- Key Insight: Separate fee fields (foreign_xyk_fee vs native_xyk_fee) preserve currency information for monitoring and analytics

### 4.7 Smart Router Path Selection

- Nature: Tests TMC vs XYK price comparison logic
- Necessity: Ensures users always receive optimal price
- Validates: Cheaper path chosen, tie-breaking to TMC

### 4.8 TMC Burn Functionality

- Nature: Tests token destruction and supply reduction
- Necessity: Confirms deflationary mechanics work correctly
- Validates: Supply decreases, price calculated before burn

---

## 5. Integration & System Flows

Multi-component workflows and system-level behavior validation.

### 5.1 Edge Cases

- Nature: Tests degenerate inputs and empty states
- Necessity: Prevents division-by-zero and null-pointer equivalents
- Validates: Zero amounts, empty pools, first mint handled gracefully

### 5.2 Full Integration Flow

- Nature: Complete user journey: mint → swap → burn
- Necessity: Reveals interaction bugs invisible in unit tests
- Validates: End-to-end flow produces consistent state
- Failure Criteria: State inconsistency, balance mismatch, or operation failure

### 5.3 Overflow Protection Testing

- Nature: Tests near-maximum uint256 operations
- Necessity: Critical for preventing financial exploits
- Validates: Safe multiplication, no silent overflows
- Failure Criteria: Overflow occurs or result wraps around

### 5.4 Safe Operating Ranges

- Nature: Identifies practical parameter boundaries
- Necessity: Establishes production deployment guidelines
- Validates: Conservative/moderate/aggressive configs all safe

### 5.5 Formula Batch Execution Smoke

- Nature: Repeats quadratic formula execution across a fixed batch
- Necessity: Keeps the correctness suite deterministic while still exercising the hot formula path at scale
- Validates: Batched formula execution produces positive output without mutating state

---

## 6. System Invariants & Multi-Actor

Validation of system invariants and participant class integration with core tokenomics. Actual distribution is User + TOL (with 4 internal buckets).

### 6.1 Distribution Accuracy - Multi-Mint Accumulation

- Nature: Economic correctness verification across multiple minting operations
- Necessity: Validates exact distribution percentages (33.3% user, 66.7% TOL) hold over time
- Validates: TOL receives precise allocation through `receive_mint_allocation()` and distributes to 4 internal buckets (50%, 16.67%, 16.67%, 16.67%); distribution ratios remain constant regardless of mint count; rounding errors remain within tolerance (<1%)
- Failure Criteria: Distribution ratio deviates >1% from configured shares; accumulation mechanism fails; TOL bucket balances incorrect

### 6.2 Mass Conservation - System-Wide Token Accounting

- Nature: Fundamental conservation law for token supply
- Necessity: Proves tokens cannot be created or destroyed outside designed mechanisms (minting, TOL lock)
- Validates: Total supply exactly equals user balances + TOL reserves (across all 4 buckets); no tokens lost to rounding; supply tracking remains consistent across all operations
- Failure Criteria: Supply mismatch >1 wei; tokens disappear or appear unexpectedly; balance sum ≠ total supply

### 6.3 TOL Independence - Participant Sales Don't Touch TOL

- Nature: Security property ensuring TOL permanence
- Necessity: Confirms TOL liquidity remains locked regardless of participant trading activity
- Validates: User sales only affect XYK reserves, not TOL LP tokens; TOL balance strictly non-decreasing (only increases on new mints); each of 4 TOL buckets maintains independent LP balances
- Failure Criteria: TOL LP balance decreases; user sales affect TOL allocation; TOL buckets become accessible to users

---

## 7. Advanced Integration Scenarios

Complex economic behaviors and advanced component interactions.

### 7.1 Circular Swaps and Arbitrage Detection

- Nature: Tests round-trip swap profitability
- Necessity: Proves system resistant to risk-free arbitrage
- Validates: Fees prevent circular profit, price convergence observed
- Additional Check: Anti-Arbitrage Cycle — repeated deterministic foreign→native→foreign cycles (e.g., 10 iterations) must be non-profitable; starting capital is non-increasing across cycles.

### 7.2 Minimum Trade Amount Enforcement

- Nature: Tests trade size restrictions
- Necessity: Prevents dust attacks and spam
- Validates: Sub-minimum trades rejected, error messages clear

### 7.3 Slippage Protection in Router

- Nature: Tests `min_output` parameter enforcement
- Necessity: Protects users from front-running attacks
- Validates: Insufficient output reverts transaction

### 7.4 TOL Buffer Behavior Before Pool Initialization

- Nature: Tests TOL accumulation when pool doesn't exist
- Necessity: Ensures smooth cold-start without liquidity
- Validates: Buffer holds tokens, flushes on pool creation

### 7.5 Fee Manager Buffer and Burn Mechanics

- Nature: Tests fee accumulation and threshold-based burning
- Necessity: Confirms deflationary mechanism activates correctly
- Validates: Fees buffered, burned when threshold reached

### 7.6 Distribution Remainder Handling

- Nature: Tests rounding remainder allocation to TOL
- Necessity: Prevents dust loss from fractional PPB in 2-way split (user 33.3% + TOL 66.7%)
- Validates: Total distributed matches minted exactly

---

## 8. System Properties & Invariants

Mathematical guarantees and system-level properties that must never break.

### 8.1 System Invariants After Heavy Use

- Nature: Tests conservation laws after 1000+ operations
- Necessity: Proves system stability under production load
- Validates:

* Total supply = sum of all balances
* XYK invariant `k` only increases (fees)
* TOL never decreases (locked forever)
* Price boundaries hold (floor ≤ market ≤ ceiling)

### 8.2 Infrastructure Premium Mathematical Proof

- Nature: Proves users receive more tokens via TMC than XYK
- Necessity: Validates "protocol arbitrage, not user taxation" claim
- Validates: TMC allocation > hypothetical XYK allocation for same payment

### 8.3 Floor Formula & Scenario Verification

- Nature: Validates the physical derivation of the floor price
- Necessity: Proves the $P_{floor} = k/(R+S)^2$ law holds in simulation
- Validates:
  - Calculated spot price matches theoretical floor after simulated dump
  - Floor/Ceiling ratio aligns with $1/(1+s/a)^2$ approximation

---

## 9. Multi-User & Chaos Testing

Emergent behavior from concurrent interactions and chaos testing.

### 9.1 Multi-User Concurrent Simulation

- Nature: Simulates 100 random users, 500+ operations
- Necessity: Reveals emergent properties invisible in isolated tests
- Validates:

* Conservation laws under chaos
* Price ratchet acceleration (floor rises)
* Deflation acceleration (burning compounds)
* System stability (no deadlocks/livelocks)

- Failure Criteria: Conservation error >0.01%, TOL balance reduction, XYK constant k decreases, system deadlock
- Key Insight: Multi-user flows expose state interactions that unit tests cannot predict.

### 9.2 Extreme Load Stress Test

- Nature: Tests system stability under extreme operational load (500+ rapid operations)
- Necessity: Validates system resilience and invariant preservation during high-frequency trading
- Validates:
  - Supply remains positive under extreme load
  - Pool maintains liquidity during rapid operations
  - K invariant remains positive and only increases (fees accumulate)
  - Fee system processes transactions under load
  - Total supply equals sum of all mints plus TOL allocation (mass conservation)
- Failure Criteria: System deadlock, invariant violation, liquidity depletion, fee processing failure
- Key Insight: Extreme load testing reveals performance bottlenecks and state management issues invisible under normal conditions.

---

## 10. Emergent Properties & System Intelligence

Tests for system behaviors that emerge from component interactions and intelligent system responses.

### 10.1 Bootstrap Gravity Well Detection

- Nature: Critical TOL accumulation threshold validation
- Necessity: Identifies the point where system transitions from fragile to stable
- Validates: System achieves stability when TOL value exceeds ~15% of market capitalization
- Failure Criteria: System remains unstable despite significant TOL accumulation

### 10.2 Supply Elasticity Inversion Point

- Nature: Tests the counterintuitive property where increasing supply raises minimum price
- Necessity: Validates TMCTOL's unique economic behavior that inverts traditional supply-demand dynamics
- Validates: After critical supply level, floor price increases despite supply expansion
- Clarification: This test targets the inversion boundary (`dF/dS = 0`) and the post-inversion rise of the effective floor. It is distinct from relative compression parity (`d ln F / d ln S = d ln C / d ln S`), from absolute-gap compression (`dF/dS > dC/dS`), and from arbitrage reversal / overtake (`P_stress ≥ P_ceiling`)
- Failure Criteria: Traditional supply-demand relationship persists (more supply → lower price)

### 10.3 Vesting Cliff Math Trap Detection

- Nature: Tests mathematical lock-in of large holder tokens during convergence
- Necessity: Confirms price impact creates natural exit difficulty
- Validates: Large holder tokens (e.g., 10%+ of supply) become mathematically difficult to exit when floor approaches ceiling; TOL liquidity must be present, the convergence gap must exist, and burn-time compression must tighten that gap rather than silently skipping the scenario
- Failure Criteria: Large holders can exit significant positions without severe price impact during convergence; the fixture produces no floor liquidity; burn-time compression fails to reduce the arithmetic gap

### 10.4 Mint-Swap Feedback Loop Analysis

- Nature: Tests self-reinforcing cycle where TMC mints degrade XYK prices
- Necessity: Validates router fee effectiveness in preventing infinite mint avalanches
- Validates: Router fees (0.5%) create sufficient friction to limit consecutive TMC routes
- Failure Criteria: Unlimited consecutive TMC routes create price manipulation vulnerability

### 10.5 Slope Efficiency Sublinearity Verification

- Nature: Tests that equilibrium price scales as √slope, not linearly
- Necessity: Validates diminishing returns from slope parameter increases
- Validates: 10x slope increase gives ~3.16x equilibrium price (√10), 100x slope gives ~10x price (√100)
- Failure Criteria: Linear scaling observed (10x slope gives 10x price)

### 10.6 Compression Threshold Taxonomy

- Nature: Tests the mathematical separation between inversion, ratio-compression, arithmetic-gap compression, and overtake
- Necessity: Prevents conflating floor recovery with corridor compression and keeps the executable mirror aligned with the spec's threshold taxonomy
- Validates: Inversion boundary keeps floor flat while ceiling still rises; post-inversion floor recovery can still widen both `C/F` and `C - F`; relative compression parity keeps `C/F` flat while arithmetic gap can still widen; absolute-gap compression narrows `C - F` before overtake; overtake requires `F ≥ C`
- Failure Criteria: Distinct threshold regimes collapse into one another; post-inversion floor growth is treated as compression by default; overtake semantics drift from the executable math

---

## 11. Economic Security & Attack Resistance

Validation of system security against economic attack vectors with realistic scenarios and economic resilience.

### 11.1 Sandwich Attack Fee Burden

- Nature: Tests that router fees make sandwich attacks economically inefficient through fee extraction
- Necessity: Validates that 0.5% router fee per swap creates sufficient friction to deter profitable MEV extraction
- Validates: Total fees exceed 0.8% of attack capital for round-trip; attacker profit margin minimal (<0.1%); TOL accumulation continues during attack scenarios
- Failure Criteria: Fees insufficient to deter attacks; attacker achieves significant profit; TOL accumulation disrupted

### 11.2 Realistic Governance Attack - Distribution Manipulation

- Nature: Tests realistic governance attack where malicious proposal changes distribution shares to extract value
- Necessity: Confirms that mathematical floor guarantee persists even under extreme governance manipulation
- Validates: Floor guarantee maintains despite extreme governance scenarios; TOL continues accumulating across all 4 buckets; large holder extraction faces severe price impact (>100% of TOL reserves); XYK invariant maintains liquidity guarantee
- Failure Criteria: Floor guarantee compromised; large holders can extract value without severe price impact; TOL accumulation stops

### 11.3 Cross-Chain Bridge Failure Resilience

- Nature: Tests that each chain maintains floor guarantee during bridge failure with realistic price divergence
- Necessity: Validates independent economic security of each chain when cross-chain communication is disrupted
- Validates: Each chain maintains positive floor price; TOL distribution prevents single-chain dominance (>70% threshold); no arbitrage opportunities without bridge functionality; sustainable TOL-to-supply ratio (>5%) per chain
- Failure Criteria: Single chain dominates TOL; arbitrage possible without bridge; chain loses floor guarantee

### 11.4 TOL Capital Efficiency vs Traditional Treasury

- Nature: Tests TMCTOL's capital efficiency advantage over traditional treasury models through continuous liquidity deployment cycles
- Necessity: Validates that ~100% capital utilization (with temporary buffers recycled into subsequent mints) provides price floor protection, value accrual, and resilience vs 0% traditional treasury
- Validates: ~100% capital utilization through deployment cycles (vs 0% traditional idle treasury) with temporary buffers for Zap operations; mathematical price floor protection (11-25% range); value accrual through XYK participation (quadratic growth capture); 50%+ resilience to strategic spending while maintaining protection
- Failure Criteria: Capital utilization significantly below 100%; floor protection fails; value accrual mechanism broken; spending eliminates protection

---

## 12. Adaptive System Behaviors

Validation of adaptive system behaviors, intelligent routing decisions, and economic incentive alignment.

### 12.1 Bidirectional Compression Effect

- Nature: Tests the simultaneous compression of price range from both directions
- Necessity: Validates that burning reduces ceiling while TOL accumulation raises floor, creating convergence
- Validates: Arithmetic spread (`C - F`) narrows after token burning; relative spread (`C/F`) narrows after token burning; ceiling decreases while floor increases or remains stable
- Failure Criteria: Arithmetic spread expands after burning; relative spread expands after burning; ceiling increases; floor decreases

### 12.2 Router Intelligence - TMC Route Selection

- Nature: Tests router's ability to select optimal TMC route when XYK lacks liquidity
- Necessity: Ensures users receive best available price by defaulting to TMC when XYK is unavailable
- Validates: Router chooses TMC route when XYK pool has no liquidity; TMC route provides positive output; system handles initial mint correctly
- Failure Criteria: Router fails to select TMC when XYK unavailable; zero output from TMC route

### 12.3 Router Intelligence - XYK Route Selection

- Nature: Tests router's ability to select optimal XYK route when established liquidity exists
- Necessity: Ensures users benefit from established liquidity pools when available
- Validates: Router chooses XYK route when pool has sufficient liquidity; XYK provides better price than TMC in established state; router correctly compares TMC vs XYK prices
- Failure Criteria: Router selects TMC when XYK provides better price; incorrect price comparison

### 12.4 Two-Phase Reward Routing - Unified Fee Collection

- Nature: Tests collection of non-router protocol fees into Fee Sink.
- Necessity: Keeps collection separate from phase-specific allocation and the Axial Router burn path.
- Validates: Fee Sink receives 100% of the collected amount.
- Failure Criteria: Any amount bypasses Fee Sink or total value is not conserved.

### 12.5 Two-Phase Reward Routing - Phase 1 Pools

- Nature: Tests trusted-collator launch redistribution from Fee Sink.
- Necessity: Validates Phase 1 routes only to staking-pool yield and LP donation flows.
- Validates: 50/50 pool split and exact conservation.
- Failure Criteria: Any missing half, extra destination, or conservation failure.

### 12.6 Two-Phase Reward Routing - Phase 2 Equal Thirds

- Nature: Tests the future permissionless-collator redistribution shape.
- Necessity: Keeps the gated Phase 2 target explicit without inventing its unresolved security-reward topology.
- Validates: Equal thirds for security rewards, staking ingress, and liquidity provisioning.
- Failure Criteria: Unequal allocations or non-conserved Fee Sink amount.

### 12.7 Two-Phase Reward Routing - Remainder Conservation

- Nature: Tests reward-routing splits with dust-sized, non-divisible amounts.
- Necessity: Prevents integer division from leaking or inventing value when fees are smaller than clean ratio units.
- Validates: Full Fee Sink collection, Phase 1, and Phase 2 all conserve total input, with indivisible equal-third remainder retained by Fee Sink for a later cycle.
- Failure Criteria: Any split loses value, creates value, or routes remainder outside the configured destination set.

### 12.8 Economic Incentive Alignment

- Nature: Tests that system rewards beneficial behaviors and penalizes harmful ones.
- Necessity: Validates economic incentives align with system stability and long-term holding.
- Validates: Arbitrage unprofitable due to router fees; holding outperforms active churn under tested conditions; TOL accumulation continues during activity.
- Failure Criteria: Arbitrage profitable; trading more profitable than holding; manipulation rewarded.

---

## 13. Reporting & Conformance Metrics

Executable vectors for durable TMCTOL reporting metrics and conformance boundaries.

### 13.1 Reported Floor Metric Scenario Ratios

- Nature: Tests the canonical reported floor ratio under named scenario assumptions.
- Necessity: Prevents the `11–25%` public range from drifting across denominators or hidden reserve scopes.
- Validates: User-exit ratio near 25%, system-exit ratio near 11.11%, and default `λ = 1` sellable pressure.
- Failure Criteria: Scenario ratio or sellable-pressure basis deviates from the canonical formula.

### 13.2 Ledger Conservation With Burns

- Nature: Tests supply accounting after mint and burn flows.
- Necessity: Separates mint-time distribution conservation from burned-supply ledger conservation.
- Validates: `cumulative_minted - cumulative_burned = live simulated supply` when initial issuance is zero.
- Failure Criteria: Live supply does not reconcile after burns.

### 13.3 Burn Liveness Threshold Behavior

- Nature: Tests discrete fee-buffer threshold behavior.
- Necessity: Makes burn-ratchet liveness executable instead of only continuous commentary.
- Validates: Sub-threshold foreign fees remain buffered; threshold-crossing fees burn native when liquidity exists.
- Failure Criteria: Premature burn, stuck threshold-crossing buffer, or missing native burn.

### 13.4 Zap Bucket Accounting Conservation

- Nature: Tests bucket-level LP/native/foreign accounting after Zap allocation.
- Necessity: Supports the floor-reporting bucket state model with executable accounting conservation.
- Validates: Sum of bucket LP/native/foreign fields equals aggregate Zap totals and Bucket_A receives classifiable LP.
- Failure Criteria: Bucket sums diverge from aggregate totals or Bucket_A cannot be classified as anchor LP.

### 13.5 Stress Floor Monotonicity

- Nature: Tests stress-floor response to larger sellable pressure.
- Necessity: Pins the canonical `P_stress(x)` direction used by floor reports.
- Validates: Larger sellable pressure lowers the stress floor for fixed reserves.
- Failure Criteria: Stress floor increases or stays equal as sellable pressure grows.

---

## Test Execution

```bash
node ./simulator/tests.js
```

- Expected Output: Minimal statistics, error codes only for failures
- Tolerance: ~0.01% for emergent behaviors (multi-step calculations)
- Coverage: 66 tests validating 13 system layers
- Documentation Standard: Each test includes Nature/Necessity/Validates/Failure Criteria

---

## Synchronization Protocol

1. Add Test: Update both `tests.js` implementation and this `tests.md` documentation with all four fields (Nature/Necessity/Validates/Failure Criteria)
2. Modify Test: Sync all four fields to match new behavior
3. Remove Test: Delete from both files, update test count in overview
4. Refactor: Maintain test ID stability for historical comparison

---

## Quality Metrics & Test Distribution

- Mathematical Foundations: 4 tests for core formulas
- System Parameters: 6 tests for boundaries and extremes
- Scaling Rules: 6 tests for precision and consistency
- Core Components: 8 tests for isolated component behavior
- Integration Flows: 5 tests for multi-component workflows
- System Invariants: 3 tests for conservation laws
- Advanced Scenarios: 6 tests for complex behaviors
- System Properties: 3 tests for mathematical guarantees
- Multi-User Testing: 2 tests for chaos and concurrency
- Emergent Properties: 6 tests for system behavior analysis
- Economic Security: 4 tests for attack resistance and capital efficiency (continuous deployment vs idle vaults)
- Adaptive System Behaviors: 8 tests for intelligent system responses and two-phase reward routing
- Reporting & Conformance Metrics: 5 tests for floor reporting, burn liveness, Zap accounting, and ledger conservation
- Philosophy: Tests prove specifications; specifications guide implementation; implementation validates mathematics.

---

## Test Codes Reference & Architecture Mapping

- Each test has a hierarchical code based on its section and position, mapping to TMCTOL architecture:

* `1.1-1.4`: Mathematical Foundations (Core Formulas)
* `2.1-2.6`: System Parameters & Boundaries
* `3.1-3.6`: Scaling Rules & Precision
* `4.1-4.8`: Core Component Tests (TMC, XYK, Router, TOL)
* `5.1-5.5`: Integration & System Flows
* `6.1-6.3`: System Invariants & Multi-Actor
* `7.1-7.6`: Advanced Integration Scenarios
* `8.1-8.3`: System Properties & Invariants
* `9.1-9.2`: Multi-User & Chaos Testing
* `10.1-10.6`: Emergent Properties & System Intelligence
* `11.1-11.4`: Economic Security & Attack Resistance
* `12.1-12.8`: Adaptive System Behaviors
* `13.1-13.5`: Reporting & Conformance Metrics

- Use these codes in error messages for quick reference to this documentation.
