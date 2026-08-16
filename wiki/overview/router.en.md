---
type: overview
title: DEOS Router
description: DEOS Router is the framework's max-output routing engine. It compares bounded route candidates, publishes pre-execution pool samples to typed standalone observations, uses the native asset as the main routing anchor, and keeps swaps on the canonical protocol path.
locale: en
canonical_page_id: router
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/router/docs/architecture.en.md
  - resource: ../../docs/oracle.integration.en.md
  - resource: ../../docs/core.architecture.en.md
  - resource: ../../template/pallets/router/docs/specification.en.md
status: stable
audience: newcomer
tags:
  - overview
  - router
  - routing
  - execution
related:
  - Typed Observations
  - Token Minting Curve
  - Routing and Minting Loop
  - TMCTOL Standard
  - Token-Driven Automation
  - Asset Identity
last_compiled: 2026-08-10
confidence: 0.9
---

# DEOS Router

## Summary

DEOS Router is the runtime's route-selection engine. Its job is not to be a general-purpose DEX aggregator, but to make a bounded protocol decision about how a swap should execute inside a DEOS-style economy.

In practice, it compares a small set of candidate paths across market liquidity and protocol liquidity, then chooses the route that delivers the most output to the swap recipient. That is pure max-output selection: no additional policy weight influences the result.

Just as important, the protocol's canonical swap path goes through the router. Swapping around it is not part of the DEOS contract, because bypassing the router would bypass route selection, fee capture, and the protocol's own economic coordination logic.

## What Makes It Different

The router is deliberately opinionated:

- It uses the native asset as the main routing anchor
- It compares XYK pool routes with mint-side protocol routes
- It publishes pre-execution pool samples to typed standalone Oracle feeds
- It verifies actual exact-input output and exact-output spend/output deltas rather than trusting quote math

That makes it a coordination layer, not just a convenience helper.

## How It Decides

The current implementation evaluates a small candidate set such as direct XYK routes, direct mint routes, and native-anchored multi-hop routes.

It ranks exact-input routes by maximum recipient output and exact-output routes by minimum total input, with deterministic family/path tie-breaking. Price impact and fee fields on quotes stay informational. Execution prepares current route truth again, validates every actual XYK leg against its directional reference, and enforces the authored output floor or total-input ceiling against measured deltas. These checks remain local protection, not a fair-price proof or flash-loan, ordering, or sandwich immunity.

Directional pool observations live in the standalone Oracle pallet. Canonical pool admission creates both typed directions with immutable producer, scale, aggregation, and provenance; the Router publishes each actual XYK leg immediately before executing it in canonical order. Direct TMC mint publishes no XYK observation. Router-local EMA and tracked-asset storage no longer exist. Generalized feeds and unbounded on-chain history remain out of scope.

The router is not optional glue around canonical product swaps. It is the reference protocol gateway for fee-bearing route comparison, while the runtime does not claim that every lower-level Asset Conversion call is technically unreachable.

## Why It Matters to TMCTOL

TMCTOL needs a way to compare ordinary pool liquidity with protocol liquidity coming from the minting curve. DEOS Router is the subsystem that performs that comparison.

That is why the router is a first-class economic actor in the architecture. It is where route choice, fee capture, and protocol-side execution meet.

## Canonical On-Chain Surface

The router exposes bounded typed on-chain quote views for exact-input and exact-output previews at an explicit block hash. That gives clients a bounded route preview directly from the chain instead of forcing the browser to reconstruct router logic off-chain. Execution never treats a quote as authority: it re-prepares current state and emits one canonical outcome containing family, legs, actual amounts, Router fee, and Weight class.

Long-range analytics and historical route dashboards still belong to materialized views, not to canonical runtime state.

## Related

- [Typed Observations](typed-observations.en.md)
- [Token Minting Curve](token-minting-curve.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [TMCTOL Standard](../concepts/tmctol-standard.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Asset Identity](asset-identity.en.md)
