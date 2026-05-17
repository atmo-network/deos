---
page_type: usage
title: Partner Evaluation Route
summary: A short five-page reading route for partner teams evaluating whether to fork DEOS and build a downstream ecosystem on top of it.
locale: en
canonical_page_id: partner-evaluation-route
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../AGENTS.md
  - ../../BACKLOG.md
  - ../../wiki/getting-started/executive-summary.en.md
status: active
audience: partner
tags:
  - onboarding
  - partners
  - adoption
  - fork
related:
  - Executive Summary
  - Partner Pitch
  - Minimal Fork Profile
  - Forking DEOS
last_compiled: 2026-05-17
confidence: 0.84
---

# Partner Evaluation Route

## Purpose

This route is for a team asking: “Should we fork DEOS for our own ecosystem?”

It keeps the first pass short. Do not start with every pallet, runtime detail, or economic threshold. Start with the adoption question, then move into the framework graph only if the fit is real.

## The five-page route

1. [DEOS in 60 Seconds](../getting-started/deos-in-60-seconds.en.md) — understand the core meme.
2. [Executive Summary](../getting-started/executive-summary.en.md) — see what is shipped, what is not shipped, and why Polkadot/Substrate matters.
3. [Partner Pitch](../getting-started/partner-pitch.en.md) — understand the partner-facing value proposition and 30/90 day shape.
4. [DEOS vs DAO Treasury](../comparisons/deos-vs-dao-treasury.en.md) — compare committee treasury management with deterministic circuits.
5. [Minimal Fork Profile](minimal-fork-profile.en.md) — check the smallest credible fork shape.

## Decision checkpoint

After those five pages, a partner team should be able to answer:

- Is our target product actually a protocol economy, not just an app?
- Do we want runtime-level economic circuits, or would a normal smart-contract stack be enough?
- Does TMCTOL fit our launch thesis, or do we need a different standard on DEOS?
- Which data must be on-chain, and which data can be materialized by an indexer?
- What user-facing product, dApps, and community narrative are ours to build downstream?

## If the answer is yes

Continue to [Forking DEOS](forking-deos.en.md), [Domain Map](../concepts/domain-map.en.md), and [Three-Layer Validation](../development/three-layer-validation.en.md).
