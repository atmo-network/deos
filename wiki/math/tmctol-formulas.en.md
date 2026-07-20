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
last_compiled: 2026-07-20
confidence: 0.85
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
- `PRECISION` = fixed-point scaling denominator; runtime overflow safety comes from checked arithmetic and wider `U256` intermediates

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

When a mint occurs, the minted token output is distributed according to the configured split:

- approximately `33.3%` to the user;
- approximately `66.6%` to the protocol sink used by the TOL topology.

The collateral payment is transferred separately to its resolved protocol destination; this ratio does not split the foreign collateral.

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
- [Axial Router](../overview/axial-router.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
