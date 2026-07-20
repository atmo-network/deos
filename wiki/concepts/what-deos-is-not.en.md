---
page_type: concept
title: What DEOS Is Not
summary: A negative-space definition of DEOS that prevents common misunderstandings about governance, liquidity, guarantees, smart contracts, indexers, and randomness.
locale: en
canonical_page_id: what-deos-is-not
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
status: active
audience: newcomer
tags:
  - concept
  - positioning
  - boundaries
related:
  - DEOS Framework Overview
  - Invariant and Threat Map
  - Economic Claim Levels
  - Parachain Context
last_compiled: 2026-07-20
confidence: 0.9
---

# What DEOS Is Not

## Summary

DEOS is easier to understand when its negative space is explicit. It is a deterministic economic framework, not a promise that every adjacent problem is solved inside the runtime.

## Not These Things

- **Not a generic DAO.** DEOS governance is domain-scoped and bounded by typed payloads, protection tracks, and execution authority.
- **Not a redemption-backed stable asset.** TMCTOL has floor-support and liquidity mechanics, not an unlimited redemption promise.
- **Not an unbounded smart-contract platform.** The runtime ships bounded economic services, not arbitrary user-deployed computation.
- **Not an oracle-free guarantee machine.** Some claims depend on reserves, market state, routing state, or future provider/read-model surfaces.
- **Not an indexerless analytics platform.** Bounded live projections can be on-chain; unbounded history and search belong to materialized providers.
- **Not a randomness/fairness product at launch.** The launch line uses trusted-collator simplification until a suitable relay/protocol randomness path is available.
- **Not a finished ecosystem product.** The repository is a forkable framework; downstream forks provide product narrative, users, dApps, and ecosystem policy.

## Why This Matters

Negative boundaries prevent overclaiming. They also tell fork teams which responsibilities they inherit instead of assuming the framework silently handles them.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Invariant and Threat Map](invariant-map.en.md)
- [Economic Claim Levels](economic-claim-levels.en.md)
- [Parachain Context](parachain-context.en.md)
