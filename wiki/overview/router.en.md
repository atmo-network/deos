---
type: overview
title: DEOS Router
description: DEOS Router selects bounded exact-input and exact-output routes, publishes pre-execution pool samples to typed standalone observations, and is the required public gateway for XYK execution.
locale: en
canonical_page_id: router
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/router/docs/architecture.en.md
  - resource: ../../docs/oracle.integration.en.md
  - resource: ../../docs/asset-conversion.integration.en.md
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
---

# DEOS Router

## Summary

DEOS Router is the runtime's route-selection engine. Its job is not to be a general-purpose DEX aggregator, but to make a bounded protocol decision about how a swap should execute inside a DEOS-style economy.

In practice, it compares a small set of candidate paths across market liquidity and protocol liquidity. Exact-input selection maximizes recipient output, while exact-output selection minimizes total caller input including the Router fee. No additional policy weight influences either ranking.

Just as important, public XYK execution goes through the router. A direct signed Asset Conversion swap is not part of the DEOS contract, because it would bypass route selection, fee capture, and the protocol's own economic coordination logic.

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

The router is not optional glue around canonical product swaps. It is the public protocol gateway for fee-bearing route comparison. The runtime call filter rejects raw Asset Conversion swaps, so signed users cannot bypass Router fee collection or route policy.

## Pool Identity and Creation

Asset identity has two layers. `AssetKind` states semantic meaning, while `LedgerAssetKey` identifies the physical balance ledger. Every pool endpoint must use its one canonical semantic representation, and the two endpoints must resolve to different physical ledgers. This prevents apparently distinct Local and Foreign values from creating a pool over the same balances.

Permissionless pool creation also enters through DEOS Router. One transaction validates the canonical pair and expected LP identity, creates the underlying XYK pool, verifies and indexes the actual LP asset, and creates both directional DEOS Oracle feeds. Failure restores all Pool, LP, account, event, and Oracle changes. Raw Asset Conversion pool creation and post-dispatch topology repair are unavailable.

Pool quotes, liquidity changes, and execution use full physical pool balances. Swap LP fees, liquidity-withdrawal fees, and the Router fee remain independent domains; the reference launch uses `0%`, `0%`, and `0.5%` respectively.

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
