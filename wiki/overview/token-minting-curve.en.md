---
page_type: overview
title: Token Minting Curve
summary: The Token Minting Curve is the mint-only issuance engine in the current TMCTOL line. It prices new supply along a deterministic linear ceiling, uses integral math for exact minting, and treats launch parameters as immutable physics on the current launch line.
locale: en
canonical_page_id: token-minting-curve
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmc.architecture.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: newcomer
tags:
  - overview
  - tmc
  - minting
  - tokenomics
related:
  - Axial Router
  - Routing and Minting Loop
  - TMCTOL Standard
  - TMCTOL Formulas
  - Physics-First vs Politics-First
last_compiled: 2026-04-16
confidence: 0.94
---

# Token Minting Curve

## Summary

The Token Minting Curve, or `TMC`, is the mint-side issuance engine of the current TMCTOL standard. It allows new supply to be created according to deterministic curve math, but it does not provide a reverse redemption path.

That is why the docs describe it as a unidirectional minting engine and as part of the protocol's launch-time physics.

## What It Does

TMC defines a linear price ceiling for new issuance. When users enter through the mint path, the pallet calculates how much supply should be created for the payment amount using integral-based math.

In simpler terms, the curve determines the price of new protocol-issued supply and makes that calculation explicit on-chain.

## Why the Curve Is One-Way

The current design intentionally allows creation without reverse extraction through the same path. That asymmetry is important to the TMCTOL economic model because it avoids treating the curve like a redeemable reserve vault.

The result is a ratchet-style issuance path: new supply can be justified mathematically, but the curve itself is not a withdrawal door.

## Current Launch-Line Contract

On the current launch line, the key curve parameters are configured when a curve is created and are then treated as immutable.

That rule matters because changing those parameters later would mean rewriting launch physics rather than merely tuning an operational setting.

## Relationship to the Rest of TMCTOL

TMC is only one part of the system. In the current reference line:

- TMC handles deterministic issuance
- The Axial Router decides when the mint path is the better route
- Mint-side protocol allocation feeds later liquidity-provisioning flows
- Treasury-owned liquidity and burn flows interact with the resulting supply dynamics

This is why TMC is important, but not sufficient by itself to explain TMCTOL.

## Canonical On-Chain Surface

The pallet exposes bounded on-chain truth for curve existence, live curve parameters, effective supply state, and mint-side execution effects.

Historical mint analytics and long-range dashboards belong to indexed or materialized views rather than to the canonical runtime contract.

## Related

- [Axial Router](axial-router.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [TMCTOL Formulas](../math/tmctol-formulas.en.md)
- [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md)
