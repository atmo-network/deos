---
page_type: concept
title: Routing and Minting Loop
summary: The current DEOS reference line pairs the Axial Router with the Token Minting Curve to decide how trades should execute and how new supply enters the system. The router compares market liquidity with protocol liquidity, while TMC provides deterministic mint-side pricing.
locale: en
canonical_page_id: routing-and-minting-loop
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/tmc.architecture.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - router
  - tmc
  - execution
related:
  - Axial Router
  - Token Minting Curve
  - TMCTOL Standard
  - Token-Driven Automation
  - Staking Pools
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.93
---

# Routing and Minting Loop

## Summary

In the current DEOS reference line, the Axial Router and the Token Minting Curve form one coordinated execution loop. The router decides which mechanism should handle a trade, and TMC supplies the deterministic mint path when protocol liquidity is the better route.

This pairing matters because TMCTOL is not just a curve and not just an AMM. It is a system where market liquidity, protocol liquidity, fee burning, and liquidity provisioning all interact.

## Axial Router Role

The Axial Router is described as a protocol-first decision engine rather than a generic DEX aggregator. Its job is to compare available routes and choose the one with the best execution under the runtime's bounded logic.

The current architecture evaluates a small candidate set, including:

- Direct XYK routes
- Direct mint routes
- Native-anchored multi-hop routes

It also updates its EMA oracle before execution so route selection is harder to manipulate inside the same block.

## TMC Role

The Token Minting Curve is the mint-only issuance engine. It prices new supply along a deterministic linear ceiling and uses integral-based math to calculate exactly how much supply should be minted for a given payment.

On the current launch line, curve parameters are configured at creation time and treated as immutable launch physics.

## How They Work Together

The router can treat TMC as one candidate execution mechanism alongside XYK pools. That lets the protocol compare:

- `Market liquidity` from pools
- `Protocol liquidity` from mint-side issuance

When the protocol path is better, the router sends execution through TMC. When the market path is better, it uses XYK routing.

## Why the Loop Matters for TMCTOL

This design gives TMCTOL a cleaner economic loop:

- The router handles route selection and fee collection
- TMC handles deterministic issuance
- Mint-side protocol allocation can be pushed into liquidity provisioning
- Router fees can be routed toward burning and supply compression

That is why the docs describe the router as an economic coordination actor rather than just a swap helper.

## Canonical On-Chain Surface

The router now exposes a typed quote view and the TMC pallet exposes bounded curve state. These are part of the live on-chain contract for route preview and minting truth.

Long-range route analytics or chart history remain separate materialized concerns rather than canonical runtime state.

## Related

- [Axial Router](../overview/axial-router.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
- [Staking Pools](staking-pools.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/axial-router.architecture.en.md`
- `docs/tmc.architecture.en.md`
- `docs/tmctol.specification.en.md`
