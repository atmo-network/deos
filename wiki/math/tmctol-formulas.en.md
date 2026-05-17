---
page_type: math
title: TMCTOL Formulas
summary: Mathematical models defining the Token Minting Curve (TMC) and Treasury-Owned Liquidity (TOL).
locale: en
canonical_page_id: tmctol-formulas
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
status: active
audience: developer
tags:
  - math
  - tmctol
  - tokenomics
related:
  - TMCTOL Standard
  - Routing and Minting Loop
last_compiled: 2026-04-15
confidence: 0.95
---

# TMCTOL Formulas

## Summary

The TMCTOL standard relies on explicit mathematical formulas to define its economic invariants. The primary models include the linear pricing curve, the quadratic integration for minting, and the asymptotic floor protection of the XYK liquidity model.

## Linear Pricing Curve (Spot Price)

The spot price $P$ of a newly minted token increases linearly with the total supply $S$.

```text
spot_price(S) = P₀ + m·S/PRECISION
```

where:

- `P₀` = initial_price (starting price of the asset)
- `m` = slope parameter (steepness of the curve)
- `S` = current supply
- `PRECISION` = denominator to prevent integer overflow in runtime arithmetic

## Quadratic Integration for Minting

To calculate the exact foreign payment required to mint a specific amount of tokens ($\Delta S$), the protocol integrates the linear price curve over the minting interval.

```text
F_required = P₀·ΔS + m·(S₀·ΔS + ΔS²/2)/PRECISION
```

where:

- `F_required` = foreign payment needed
- `S₀` = supply before mint
- `ΔS` = tokens to mint

This deterministic pricing guarantees that bulk purchases are priced fairly according to the curve without slippage outside the mathematical integral.

## Distribution Ratio

When a mint occurs, the generated foreign liquidity is distributed according to the invariant split:

- `33.3%` User (Minted Token Allocation)
- `66.6%` TOL (Treasury-Owned Liquidity)

## XYK Constant Product (Floor Protection)

The generated TOL is injected into an XYK AMM pool, which mathematically guarantees that the price never reaches zero.

```text
XYK Invariant: k = R_native × R_foreign (constant)

After selling ΔS native tokens:
R_native' = R_native + ΔS
R_foreign' = k / R_native'

Price = R_foreign' / R_native'
```

Because $R_{foreign}'$ approaches zero asymptotically, the price can deteriorate indefinitely but never actually reaches zero for any finite $\Delta S$.

## Equilibrium and Backing Metrics

The theoretical backing price $P_{backing}$ where the curve-implied market cap equals foreign reserves is:

```text
P_backing ≈ √(R_foreign × m / PRECISION)
```

This is a reference point and demonstrates that governance parameter changes (like slope `m` or TOL routing `R_foreign`) directly alter the structural backing targets.

## Supply Dynamics (Compression)

The net supply trajectory is a battle between emission and burning:

```text
dS/dt = mint_rate - burn_rate
burn_rate = f_router × V_trade
```

Burning reduces the supply ($S$), which simultaneously lowers the price ceiling on the TMC curve and increases the absolute floor support in the XYK pool, compressing the spread.

## Related

- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [Axial Router](../overview/axial-router.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
