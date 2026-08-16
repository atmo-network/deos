---
type: concept
title: Routing and Minting Loop
description: The current DEOS reference line pairs DEOS Router with the Token Minting Curve to decide how trades execute and how new supply enters the system. The router compares recipient output across market liquidity and protocol liquidity, while TMC provides deterministic mint-side pricing.
locale: en
canonical_page_id: routing-and-minting-loop
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/router/docs/architecture.en.md
  - resource: ../../template/pallets/tmc/docs/architecture.en.md
  - resource: ../../docs/tmctol.specification.en.md
  - resource: ../../template/pallets/router/docs/specification.en.md
status: stable
audience: newcomer
tags:
  - concept
  - router
  - tmc
  - execution
related:
  - DEOS Router
  - Token Minting Curve
  - TMCTOL Standard
  - Token-Driven Automation
  - Staking
  - Core Terms
last_compiled: 2026-08-10
confidence: 0.9
---

# Routing and Minting Loop

## Summary

In the current DEOS reference line, DEOS Router and the Token Minting Curve form one coordinated execution loop. The router decides which mechanism should handle a trade, and TMC supplies the deterministic mint path when protocol liquidity is the better route.

This pairing matters because TMCTOL is not just a curve and not just an AMM. It is a system where market liquidity, protocol liquidity, fee burning, and liquidity provisioning all interact.

## DEOS Router Role

DEOS Router is described as a protocol-first decision engine rather than a generic DEX aggregator. Its job is to compare available routes and choose the one that delivers the most output to the swap recipient under the runtime's bounded logic.

The current architecture evaluates a small candidate set, including:

- Direct XYK routes
- Direct mint routes
- Native-anchored multi-hop routes

It publishes and validates each actual XYK leg in execution order, re-prepares stale quote projections from current state, and verifies exact-input output plus exact-output spend/output through measured deltas. This is shipped bounded protection, not a blanket guarantee that market manipulation disappears.

## TMC Role

The Token Minting Curve is the mint-only issuance engine. It prices new supply along a deterministic linear ceiling and uses integral-based math to calculate exactly how much supply should be minted for a given payment.

On the current launch line, curve parameters are configured at creation time and treated as immutable launch physics.

## How They Work Together

The router can treat TMC as one candidate execution mechanism alongside XYK pools. That lets the protocol compare:

- `Market liquidity` from pools
- `Protocol liquidity` from mint-side issuance

When the protocol path is better by delivered recipient output, the router sends execution through TMC. When the market path is better, it uses XYK routing.

## Why the Loop Matters for TMCTOL

This design gives TMCTOL a cleaner economic loop:

- The router handles route selection and fee collection
- TMC handles deterministic issuance
- Mint-side protocol allocation can be pushed into liquidity provisioning
- Router fees can be routed toward burning and supply compression

That is why the docs describe the router as an economic coordination actor rather than just a swap helper.

## Canonical On-Chain Surface

The router exposes bounded exact-input and exact-output quote views at an explicit state hash and the TMC pallet exposes bounded curve state. These are part of the live on-chain contract for route preview and minting truth.

Long-range route analytics or chart history remain separate materialized concerns rather than canonical runtime state.

## Related

- [DEOS Router](../overview/router.en.md)
- [Token Minting Curve](../overview/token-minting-curve.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
- [Staking](../overview/staking.en.md)
- [Core Terms](../glossary/core-terms.en.md)
