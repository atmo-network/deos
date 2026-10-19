---
type: math
title: TMCTOL Formulas
description: Mathematical models defining the Token Minting Curve (TMC) and Treasury-Owned Liquidity (TOL).
locale: en
canonical_page_id: tmctol-formulas
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../../docs/deos/tmctol.specification.en.md
  - resource: ../../../docs/deos/builder-economy.contract.en.md
  - resource: ../../../simulators/tmctol/README.md
status: stable
audience: developer
tags:
  - math
  - tmctol
  - tokenomics
related:
  - TMCTOL Standard
  - Routing and Minting Loop
---

# TMCTOL Formulas

## Summary

The TMCTOL standard relies on explicit mathematical formulas to define its economic invariants. The primary models include the linear pricing curve, the quadratic integration for minting, and the asymptotic floor protection of the XYK liquidity model.

## Linear Pricing Curve (Marginal Price)

The curve marginal price `P` is the cost per unit of total issuance, before distribution to the recipient and TOL. It increases with curve-accounted minted supply `S`, which need not equal live issuance after burns.

```text
spot_price(S) = P₀ + m·S/PRECISION
```

where:

- `P₀` = initial price, scaled by `PRECISION`
- `m` = stored slope, scaled by `PRECISION`
- `S` = curve-accounted supply as a token balance scaled by `PRECISION` in this fixed-point expression
- `PRECISION` = fixed-point scaling denominator; runtime overflow safety comes from checked arithmetic and wider `U256` intermediates

## Quadratic Integration for Minting

Use unscaled token quantities and coefficients to make the continuous integral's units explicit. Define `p₀ = P₀/PRECISION` in foreign/native and `μ = m/PRECISION` in foreign/native²:

```text
F_required = p₀·ΔS + μ·(S₀·ΔS + ΔS²/2)
```

Here `S₀` is curve-accounted supply before mint, `ΔS` is total new issuance, and `F_required` is payment in whole foreign tokens. Both terms have foreign-token units. Executable balances also have their own fixed-point scale; integer execution applies its specified rounding rather than treating this continuous formula as bit-exact.

For recipient share `u`, recipient output is approximately `u·ΔS`. The marginal recipient cost is `p(S)/u`, whereas the finite-trade recipient average is `F_required/(u·ΔS)`. Neither equals the curve marginal `p(S)` in general, and neither is automatically an XYK spot price.

## Distribution Ratio

When a mint occurs, the minted token output is distributed according to the configured split:

- approximately `33.3%` to the user;
- approximately `66.6%` to the protocol sink used by the TOL topology.

The collateral payment is transferred separately to its resolved protocol destination; this ratio does not split the foreign collateral.

## DEOS Reference: Second-Order `$BLDR` Allocation

The anchor/treasury TOL component of second-order `$BLDR` TMCTOL divides the protocol mint allocation equally between anchor liquidity and BLDR Treasury. With total issuance `M`, recipient output `U`, protocol output `T = M - U`, anchor allocation `A`, and direct treasury allocation `D`:

```text
A = T - floor(T/2)
D = floor(T/2)
U + A + D = M
```

All `$NTVE` collateral is assigned to the anchor-liquidity lane. The first-order reference also assigns half of its two-thirds protocol issuance to Bucket A, so both orders direct approximately `M/3` issuance to immutable anchor liquidity. Their collateral rules differ: first-order Bucket A receives `C/2`, while BLDR Anchor receives all collateral `C`.

Parent Bucket B recycling is route-dependent. If market acquisition returns `Q` existing `$BLDR`:

```text
burn = floor(Q/2)
recycled_treasury = Q - burn
net_issuance_change = -burn
```

If the Router selects the `$BLDR` TMC and full new issuance is `M` with recipient output `U`:

```text
burn = floor(U/2)
recycled_treasury = U - burn
net_issuance_change = M - burn
treasury_change = direct_treasury_allocation + recycled_treasury
```

The same acquisition policy can therefore contract supply through XYK or expand collateralized supply through TMC while funding both the immutable anchor and treasury.

## XYK Constant Product (Floor Protection)

In the idealized positive-reserve XYK model, protocol-owned liquidity creates an asymptotic price curve that stays above zero for any finite sale. This statement does not guarantee a market price or pool liveness.

```text
XYK Invariant: k = R_native × R_foreign (constant)

After selling ΔS native tokens:
R_native' = R_native + ΔS
R_foreign' = k / R_native'

Price = R_foreign' / R_native'
```

Because $R_{foreign}'$ approaches zero asymptotically, the price can deteriorate indefinitely but never actually reaches zero for any finite $\Delta S$.

## Canonical Reported Floor

A public floor report applies a named sellable-pressure fraction `λ` to a stated supply basis and compares the stressed price with an explicit reference price:

```text
x_reported = λ_reported · S_support_scope
P_stress(x) = k / (R_native + x)²
P_xyk_ref = R_foreign / R_native
reported_floor_ratio = P_stress(x_reported) / P_reference
Default P_reference = P_xyk_ref
```

The report counts only the current proportional reserve claim of positive-LP anchor or explicitly active-support positions. Dormant LP does not count until explicitly activated. Historical contribution fields are cost-basis telemetry and cannot be added to the same live pool reserves. Missing live Bucket A anchor support sets `governance_state` to `degraded`. Named pressure presets derive from configured allocation shares rather than assumed default percentages.

The report must name the reference price, units and pre/post-trade state as well as reserve scope, bucket state, supply basis, pressure and governance state. Curve marginal, recipient-adjusted marginal and finite-trade average are different normalizations.

## Conditional Selloff Scenarios

For fee-free XYK with fixed counted reserves during the sale, let `a = R_native/S_total` and `s = S_sold/S_total`:

```text
P_stress / P_xyk_ref = 1 / (1 + s/a)²
P_stress / P_curve = (P_xyk_ref / P_curve) / (1 + s/a)²
```

| Scenario | Counted native / issuance | Sold / issuance | Post-sale spot / pre-sale XYK spot |
| --- | --- | --- | --- |
| All TOL retained, public exit | 2/3 | 1/3 | 4/9 |
| Anchor only, public exit | 1/3 | 1/3 | 1/4 |
| Anchor only, public plus withdrawn native exit | 1/3 | 2/3 | 1/9 |

Anchor-only states first withdraw the non-Anchor half of LP proportionally: both reserves halve, pre-sale spot is preserved, and withdrawn foreign leaves support. The last row also sells the withdrawn native. The all-TOL row assumes all listed reserves qualify as counted support; dormant LP is not automatically eligible. These are model states, not a universal 11–25% floor or bounds across all governance choices. Fees, integer rounding and other reserve changes require separate accounting.

For a fee-free empty-pool genesis seed, total mint `Q` costs `F = p₀·Q + μ·Q²/2`. With recipient share `u` and TOL share `t = 1-u`, reserves are `R_native = t·Q` and `R_foreign = F`. Thus `P_xyk_ref = (p₀ + μ·Q/2)/t`, unlike end-of-mint curve marginal `p₀ + μ·Q`, recipient marginal `(p₀ + μ·Q)/u`, or recipient average `F/(u·Q)`. For slope-dominated genesis and `t = 2/3`, XYK/curve tends to `3/4`: the all-TOL public-exit ratio against the curve tends to `1/3`, not `4/9`.

## Equilibrium and Backing Metrics

The theoretical backing price $P_{backing}$ where the curve-implied market cap equals foreign reserves is:

```text
P_backing ≈ √(R_foreign × m / PRECISION)
```

This is an analytical reference point, not a runtime quote. Launch-time Economic Physics such as slope `m` is immutable on the current line; reserve scope and bucket policy must be stated explicitly when using a backing metric.

## Supply Dynamics (Compression)

A simplified supply identity is:

```text
dS/dt = completed_mint_rate - completed_burn_rate
```

Router fee volume can fund the Burn Actor, but actual burning depends on that actor remaining funded, configured, schedulable, and able to execute. With fixed reserves, burning does not itself change the current XYK spot price. It can improve a named stress-floor envelope only under explicit assumptions about counted reserves and sellable supply; ceiling, relative parity, absolute-gap compression, and arbitrage overtake remain distinct metrics.

## Related

- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [DEOS Router](../overview/router.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
