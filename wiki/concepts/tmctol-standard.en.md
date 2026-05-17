---
page_type: concept
title: TMCTOL Standard
summary: TMCTOL is the current flagship tokenomic standard running on DEOS. It combines a mint-only curve, treasury-owned liquidity, and fee burning to create a more explicit and rule-bound downside structure than a conventional token launch.
locale: en
canonical_page_id: tmctol-standard
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../README.md
status: active
audience: newcomer
tags:
  - concept
  - tokenomics
  - tmctol
related:
  - DEOS Framework Overview
  - Token Minting Curve
  - Axial Router
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance Domains
  - Staking Pools
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.94
---

# TMCTOL Standard

## Summary

TMCTOL means `Token Minting Curve + Treasury-Owned Liquidity`. It is not the repository name. It is the current economic standard implemented on top of DEOS.

At a high level, TMCTOL combines three ideas: mint-only issuance, protocol-controlled liquidity accumulation, and fee burning. Together, those mechanisms are meant to replace a large part of discretionary treasury management with more explicit economic rules.

## The Three Core Mechanisms

### Mint-Only Issuance

The TMC side gives the system a deterministic price ceiling for newly issued supply. Tokens can be minted along the curve, but the curve is not a redemption door back out.

That one-way design matters because TMCTOL does not want the curve to behave like an extractable reserve vault.

### Treasury-Owned Liquidity

A fixed share of mint output is routed into protocol-controlled XYK liquidity. This is the TOL side of the design.

In plain language, that means the protocol keeps building its own liquidity base instead of depending only on outside LPs. That is what gives the model its floor-support story.

### Fee Burning

The router takes a fee on swaps and burns it. Burning reduces supply and helps tighten the gap between the curve ceiling and the liquidity floor over time.

## Mint Distribution Rule

The current standard uses a two-way mint split:

- `33.3%` to the user side
- `66.6%` to TOL

This matters because the floor-support properties depend on liquidity accumulation keeping pace with supply growth.

## Bucket Model

The TOL share is split into four buckets:

- `Bucket A` for anchor liquidity and hard floor support
- `Bucket B` for building-budget and buyback-oriented flows
- `Bucket C` for capital reserve and treasury operations
- `Bucket D` for dormant policy reserve

The bucket model lets governance steer deployment without treating all protocol liquidity as interchangeable.

## What the Guarantee Depends On

TMCTOL does not claim unconditional safety. Its protections depend on governance preserving the system rules that make the model work, especially:

- Keeping TOL liquidity in the pools
- Preserving the allocation ratios
- Keeping fee burning live
- Protecting the anchor role of Bucket A

Under those conditions, the docs describe a floor-support range shaped by bucket usage rather than an unlimited downside profile.

## Why TMCTOL Runs on DEOS

TMCTOL needs more than formulas. It also needs deterministic runtime execution, routing, actors, governance, staking, and honest client-side read models.

DEOS provides that wider operating layer. TMCTOL is the current standard; DEOS is the system it runs on.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [Axial Router](../overview/axial-router.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [Governance Domains](governance-domains.en.md)
- [Staking Pools](staking-pools.en.md)
- [Core Terms](../glossary/core-terms.en.md)
