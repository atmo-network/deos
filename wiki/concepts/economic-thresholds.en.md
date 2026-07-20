---
page_type: concept
title: Economic Thresholds
summary: A wiki-level explanation of the main TMCTOL economic threshold concepts, including Gravity Well, Elasticity Inversion, compression, floor/ceiling spread, and why these claims must name their metric.
locale: en
canonical_page_id: economic-thresholds
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../simulator/README.md
  - ../../AGENTS.md
status: active
audience: newcomer
tags:
  - concept
  - economics
  - tmctol
  - thresholds
related:
  - TMCTOL Standard
  - TMCTOL Formulas
  - Token Minting Curve
  - Routing and Minting Loop
  - Token Surfaces
last_compiled: 2026-07-20
confidence: 0.85
---

# Economic Thresholds

## Summary

TMCTOL uses several economic threshold ideas. They are easy to confuse, so the wiki treats them as separate concepts rather than one vague “compression” claim.

The short rule: always ask which axis and which metric a claim uses.

## Gravity Well

`Gravity Well` is the emergent stabilization zone where treasury-owned liquidity becomes large enough relative to market capitalization to dampen volatility. It is not a magic price guarantee. It means protocol-owned liquidity has become economically meaningful enough to change the behavior of the system.

The exact strength of this effect depends on reserves, circulating supply, curve parameters, and market/liquidity behavior.

## Elasticity Inversion

`Elasticity Inversion` is the threshold where supply expansion stops worsening the effective floor. Before this threshold, more supply can dilute floor support. After it, additional reserve accumulation can offset or dominate that dilution.

This is an expanding-supply threshold, not the same as a burn-time compression claim.

## Compression Claims

A compression claim must name two things:

1. **Axis**: burn-time or expanding-supply.
2. **Metric**: relative spread `C/F` or absolute gap `C - F`.

Where:

- `C` is the curve-implied ceiling or mint-side price reference.
- `F` is the floor or backing-side support reference.

Burning can lower the ceiling while floor support stays stable or improves, so burn-time compression is direct. Expanding-supply analysis is different: floor recovery after inversion does not automatically mean every compression metric improves.

## Why This Matters

Without metric discipline, four different ideas get collapsed into one phrase:

- Elasticity inversion;
- Relative compression parity;
- Absolute-gap compression;
- Arbitrage reversal or overtake.

Those are not interchangeable. A page, simulator test, or governance argument that says “compression” without metric context is incomplete.

## Related

- [TMCTOL Standard](tmctol-standard.en.md)
- [TMCTOL Formulas](../math/tmctol-formulas.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [Token Surfaces](token-surfaces.en.md)
