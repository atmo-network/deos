---
page_type: overview
title: Axial Router
summary: The Axial Router is DEOS's protocol-first routing engine. It compares bounded route candidates by recipient output across market liquidity and protocol liquidity, updates its oracle before direct execution, uses the native asset as the main routing anchor, and keeps swaps on the canonical protocol path.
locale: en
canonical_page_id: axial-router
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - router
  - routing
  - execution
related:
  - Token Minting Curve
  - Routing and Minting Loop
  - TMCTOL Standard
  - Token-Driven Automation
  - Asset Identity
last_compiled: 2026-06-25
confidence: 0.95
---

# Axial Router

## Summary

The Axial Router is the runtime's route-selection engine. Its job is not to be a general-purpose DEX aggregator, but to make a bounded protocol decision about how a swap should execute inside a DEOS-style economy.

In practice, it compares a small set of candidate paths across market liquidity and protocol liquidity, then chooses the route that delivers the most output to the swap recipient under the runtime's rules.

Just as important, the protocol's canonical swap path goes through the router. Swapping around it is not part of the DEOS contract, because bypassing the router would bypass route selection, fee capture, and the protocol's own economic coordination logic.

## What Makes It Different

The router is deliberately opinionated:

- It uses the native asset as the main routing anchor
- It compares XYK pool routes with mint-side protocol routes
- It updates its EMA oracle before direct execution paths
- It verifies exact-input outcomes by recipient balance delta rather than by quote math alone

That makes it a coordination layer, not just a convenience helper.

## How It Decides

The current implementation evaluates a small candidate set such as direct XYK routes, direct mint routes, and native-anchored multi-hop routes.

It then ranks those routes by actual recipient output and executes the best one. The point is not to search an unbounded graph. The point is to give the protocol one deterministic and inspectable route-selection mechanism.

That also means the router is not optional glue around swaps. It is the required protocol gate for swap execution on the current line.

## Why It Matters to TMCTOL

TMCTOL needs a way to compare ordinary pool liquidity with protocol liquidity coming from the minting curve. The Axial Router is the subsystem that performs that comparison.

That is why the router is a first-class economic actor in the architecture. It is where route choice, fee capture, and protocol-side execution meet.

## Canonical On-Chain Surface

The router exposes a typed on-chain quote view for exact-input previews. That gives clients a bounded route preview directly from the chain instead of forcing the browser to reconstruct router logic off-chain. The execution path still verifies the delivered recipient amount, so the quote is a preview rather than the final proof of outcome.

Long-range analytics and historical route dashboards still belong to materialized views, not to canonical runtime state.

## Related

- [Token Minting Curve](token-minting-curve.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Asset Identity](asset-identity.en.md)
